//! 认证库 `auth.sqlite3`（v0.2 阶段 C，C2）：所有者凭据与会话。
//!
//! 它在数据目录里，与资料库分开，资料库备份不含它。服务器、`rssr-web passwd` 与
//! `rssr-web sessions revoke-all` 各自打开它（WAL，带 busy timeout），每次写入都是一条语句或一个
//! 短事务。
//!
//! - 所有者凭据：Argon2 哈希、凭据代数（每次改密码加一），以及登录限速用的设备标记密钥
//!   （改密码不换它：设备标记只用来区分限速计数，不是凭据）。
//! - 会话：不透明令牌，库里只存它的摘要，以及创建时间、最后记录的使用时间与创建时的凭据代数。
//!   CSRF 令牌由会话令牌派生（[`SessionToken::csrf`]）：每个会话一个、固定不变，不另存。
//!
//! 时间一律是调用方传入的 Unix 秒，测试因此能控制时钟。服务器时钟可信（NTP 同步）是前提。
//! 会话的规则（决定 Q4，[`Stored::verdict`]）：
//! - 最后使用时间最多每 [`RECORD_GRANULARITY`] 写一次，且从不往回改；
//! - 空闲判定在 [`IDLE`] 上加记录粒度：会话不会提前过期，最多晚一个粒度过期；
//! - 绝对上限 [`ABSOLUTE`]，按创建时间计，不依赖浏览器丢弃 cookie；
//! - 当前时间早于最后使用时间超过 [`CLOCK_TOLERANCE`]（时钟回拨）时拒绝；
//! - 任何拒绝都删除该会话，之后的时钟变化不能让它复活；续期只更新仍存在的行。
//!
//! 写入失败时偏于关闭：
//! - 删除失败的会话记在本进程的待吊销集合里，一直拒绝，再次出示时重试删除；
//! - 需要续期却写不进使用时间，校验返回错误，请求被拒绝（这次使用没有记录就不放行）；
//! - 待吊销集合不跨重启，所以服务器运行期间在数据目录放一个运行标记（[`RUN_MARKER`]），
//!   启动时就落盘，不依赖出错那一刻还能写入。正常停止且没有待吊销的会话时才删除它；启动时
//!   发现它（上次异常退出，或停止时仍有删不掉的失效会话），先吊销全部会话
//!   （[`AuthStore::begin_run`]、[`AuthStore::end_run`]）。代价：异常退出之后，所有浏览器都要
//!   重新登录。
//! - 服务器启动时清理已经失效的会话（[`AuthStore::purge_expired`]），认证库不可写时启动失败。

use std::collections::HashSet;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rand_core::{OsRng, RngCore};
use rusqlite::{Connection, OpenFlags, OptionalExtension, TransactionBehavior, params};
use sha2::{Digest, Sha256};
use tracing::warn;

/// 认证库在数据目录里的文件名。
pub(crate) const AUTH_DB: &str = "auth.sqlite3";
/// 服务器的运行标记：存在即“上次运行没有确认所有吊销都已落盘”。
pub(crate) const RUN_MARKER: &str = "auth.sqlite3-running";

const MINUTE: i64 = 60;
const HOUR: i64 = 60 * MINUTE;
const DAY: i64 = 24 * HOUR;
/// 空闲期限：这么久没有使用，会话过期。
pub(crate) const IDLE: i64 = 30 * DAY;
/// 最后使用时间的记录粒度：距上次记录不足这么久的使用不写库。
pub(crate) const RECORD_GRANULARITY: i64 = HOUR;
/// 绝对上限：从创建起算，不论是否一直在用。
pub(crate) const ABSOLUTE: i64 = 180 * DAY;
/// 时钟回拨的容忍量：当前时间比最后使用时间早得更多时，会话被拒绝并删除。
pub(crate) const CLOCK_TOLERANCE: i64 = 5 * MINUTE;

/// 服务器与命令行子命令可能同时打开认证库。
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

const SCHEMA_VERSION: i64 = 1;
const SCHEMA: &str = "
CREATE TABLE owner (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    password_hash TEXT NOT NULL,
    generation INTEGER NOT NULL,
    device_key BLOB NOT NULL
) STRICT;
CREATE TABLE sessions (
    digest BLOB PRIMARY KEY,
    created_at INTEGER NOT NULL,
    last_used_at INTEGER NOT NULL,
    generation INTEGER NOT NULL
) STRICT, WITHOUT ROWID;
";

const TOKEN_BYTES: usize = 32;
/// 32 字节的 base64url（无填充）长度。
const TOKEN_LEN: usize = 43;

/// 会话令牌：cookie 里放的就是它。32 字节随机数的 base64url。
///
/// 不实现 `Debug`：令牌不进日志。
pub(crate) struct SessionToken(String);

impl SessionToken {
    fn generate() -> Self {
        let mut bytes = [0_u8; TOKEN_BYTES];
        OsRng.fill_bytes(&mut bytes);
        Self(URL_SAFE_NO_PAD.encode(bytes))
    }

    /// 解析 cookie 的值。形状不对的值（例如 v0.1 的 HMAC 会话 cookie）直接拒绝，不查库。
    pub(crate) fn parse(value: &str) -> Option<Self> {
        let well_formed = value.len() == TOKEN_LEN
            && URL_SAFE_NO_PAD.decode(value).is_ok_and(|bytes| bytes.len() == TOKEN_BYTES);
        well_formed.then(|| Self(value.to_owned()))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    /// 本会话的 CSRF 令牌。由令牌经另一个用途标签的哈希派生：拿到它推不出会话令牌，
    /// 只拿到库里的摘要也算不出它。
    pub(crate) fn csrf(&self) -> String {
        URL_SAFE_NO_PAD.encode(tagged_hash(b"rssr-csrf-v1:", &self.0))
    }

    fn digest(&self) -> [u8; 32] {
        tagged_hash(b"rssr-session-v1:", &self.0)
    }
}

fn tagged_hash(tag: &[u8], token: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(tag);
    hasher.update(token.as_bytes());
    hasher.finalize().into()
}

/// 一次登录读到的凭据：哈希与凭据代数来自同一次读取。登录用这个哈希核对密码，写入会话时
/// 再核对代数未变（[`AuthStore::create_session`]）。
pub(crate) struct Credential {
    pub(crate) password_hash: String,
    generation: i64,
}

/// 库里的一个会话，连同所有者当前的凭据代数。
#[derive(Debug, Clone, Copy)]
struct Stored {
    created_at: i64,
    last_used_at: i64,
    generation: i64,
    current_generation: i64,
}

#[derive(Debug, PartialEq, Eq)]
enum Verdict {
    /// 拒绝并删除。
    Reject(Rejection),
    /// 有效；`renew` 表示该写入新的最后使用时间。
    Valid { renew: bool },
}

#[derive(Debug, PartialEq, Eq)]
enum Rejection {
    ClockSetBack,
    /// 不可能的记录（创建晚于最后使用、凭据代数不为正）：不按它计算期限。
    Corrupt,
    CredentialChanged,
    Absolute,
    Idle,
}

impl Stored {
    /// 会话在 `now` 是否有效。
    ///
    /// 不变式：库里的 `last_used_at` 不晚于最后一次被放行的使用，且每次被放行的使用 `t` 满足
    /// `t < last_used_at + RECORD_GRANULARITY`：再晚的使用要么续期成功，要么不被放行
    /// （[`AuthStore::validate`]）。因此空闲期限 `last_used_at + IDLE + RECORD_GRANULARITY` 晚于
    /// `t + IDLE`（不提前过期），且不晚于 `t + IDLE + RECORD_GRANULARITY`（最多晚一个粒度）。
    /// 差值用饱和减法，库里的异常值不会溢出。
    fn verdict(&self, now: i64) -> Verdict {
        if self.last_used_at.saturating_sub(now) > CLOCK_TOLERANCE {
            return Verdict::Reject(Rejection::ClockSetBack);
        }
        if self.created_at > self.last_used_at || self.generation < 1 {
            return Verdict::Reject(Rejection::Corrupt);
        }
        if self.generation != self.current_generation {
            return Verdict::Reject(Rejection::CredentialChanged);
        }
        if now.saturating_sub(self.created_at) >= ABSOLUTE {
            return Verdict::Reject(Rejection::Absolute);
        }
        let idle = now.saturating_sub(self.last_used_at);
        if idle >= IDLE + RECORD_GRANULARITY {
            return Verdict::Reject(Rejection::Idle);
        }
        Verdict::Valid { renew: idle >= RECORD_GRANULARITY }
    }
}

/// 打开的认证库。一个连接，操作都很短，调用方在阻塞线程上调用。
pub(crate) struct AuthStore {
    inner: Mutex<Inner>,
    run_marker: PathBuf,
}

struct Inner {
    connection: Connection,
    /// 已判定失效、删除却失败的会话摘要。本进程内一直拒绝它们，再次出示时重试删除。
    pending_revocations: HashSet<[u8; 32]>,
}

impl Inner {
    /// 删除失效的会话。删除失败时记住它：在删除成功之前，本进程不再接受它。
    fn revoke(&mut self, digest: &[u8; 32]) -> Result<()> {
        match delete_session(&self.connection, digest) {
            Ok(()) => {
                self.pending_revocations.remove(digest);
                Ok(())
            }
            Err(error) => {
                self.pending_revocations.insert(*digest);
                Err(error)
            }
        }
    }
}

impl AuthStore {
    /// 打开（必要时创建）`data_dir` 里的认证库。数据目录不存在时以仅所有者可访问的权限创建。
    ///
    /// Unix 上数据目录与认证库都必须属于当前用户、不能被其他用户写入，认证库不能被其他用户
    /// 读取；否则报错，不放宽。认证库不能是符号链接。
    pub(crate) fn open(data_dir: &Path) -> Result<Self> {
        let path = prepare_file(data_dir)?;
        let mut connection = Connection::open_with_flags(
            &path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX
                | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )
        .with_context(|| format!("无法打开认证库 {}", path.display()))?;
        connection.busy_timeout(BUSY_TIMEOUT)?;
        // 不支持共享内存的文件系统上 SQLite 保留原来的日志模式，照样正确，只是并发差一些。
        let _: String = connection.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
        // 改密码与吊销会话在提交后必须落盘。
        connection.pragma_update(None, "synchronous", "FULL")?;
        migrate(&mut connection).with_context(|| format!("无法初始化认证库 {}", path.display()))?;
        Ok(Self {
            inner: Mutex::new(Inner { connection, pending_revocations: HashSet::new() }),
            run_marker: data_dir.join(RUN_MARKER),
        })
    }

    /// 服务器开始运行。上次的运行标记还在（异常退出，或停止时仍有删不掉的失效会话）时先吊销
    /// 全部会话，返回 `true`；然后放下本次的标记（文件与目录都同步到磁盘）。之后即使写入失败、
    /// 进程又异常退出，下次启动也会吊销全部会话，判过期的会话不会因为时钟变化而复活。
    pub(crate) fn begin_run(&self) -> Result<bool> {
        let unclean = match std::fs::symlink_metadata(&self.run_marker) {
            Ok(metadata) if metadata.file_type().is_file() => true,
            Ok(_) => bail!("运行标记 {} 不是普通文件", self.run_marker.display()),
            Err(error) if error.kind() == ErrorKind::NotFound => false,
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("无法读取运行标记 {}", self.run_marker.display()));
            }
        };
        if unclean {
            self.revoke_all().context("上次运行没有正常结束，吊销全部会话失败")?;
        }
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
        }
        let marker = options
            .open(&self.run_marker)
            .with_context(|| format!("无法写入运行标记 {}", self.run_marker.display()))?;
        marker.sync_all()?;
        sync_directory(&self.run_marker)?;
        Ok(unclean)
    }

    /// 服务器正常停止。先重试删除待吊销的会话；全部删掉才移除运行标记，返回 `true`。还有删不掉
    /// 的就保留标记：下次启动吊销全部会话。
    pub(crate) fn end_run(&self) -> Result<bool> {
        let mut inner = self.lock();
        let pending: Vec<[u8; 32]> = inner.pending_revocations.iter().copied().collect();
        for digest in &pending {
            if let Err(error) = inner.revoke(digest) {
                warn!(%error, "停止时仍无法删除待吊销的会话");
            }
        }
        if !inner.pending_revocations.is_empty() {
            return Ok(false);
        }
        drop(inner);
        std::fs::remove_file(&self.run_marker)
            .with_context(|| format!("无法删除运行标记 {}", self.run_marker.display()))?;
        sync_directory(&self.run_marker)?;
        Ok(true)
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        // 持锁时 panic 的语句已随事务回滚，连接本身仍可用。
        self.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// 是否已有所有者凭据。
    pub(crate) fn has_owner(&self) -> Result<bool> {
        let found: Option<i64> = self
            .lock()
            .connection
            .query_row("SELECT 1 FROM owner", [], |row| row.get(0))
            .optional()?;
        Ok(found.is_some())
    }

    /// 还没有所有者凭据时，用 `seed` 给出的哈希建立它（凭据代数 1，新的设备标记密钥），返回
    /// 是否由这次调用建立。`seed` 只在确实没有凭据时调用；同时有别的进程建立时，先到者为准。
    pub(crate) fn initialize(&self, seed: impl FnOnce() -> Result<String>) -> Result<bool> {
        if self.has_owner()? {
            return Ok(false);
        }
        let password_hash = seed()?;
        let mut inner = self.lock();
        let transaction =
            inner.connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let inserted = transaction.execute(
            "INSERT INTO owner (id, password_hash, generation, device_key) VALUES (1, ?1, 1, ?2)
             ON CONFLICT (id) DO NOTHING",
            params![password_hash, random_key()],
        )?;
        if inserted == 1 {
            // 没有凭据时留下的会话（恢复不完整的库）不属于任何凭据：建立凭据时一并清除，
            // 否则它们恰好对上新凭据的代数 1。
            transaction.execute("DELETE FROM sessions", [])?;
        }
        transaction.commit()?;
        Ok(inserted == 1)
    }

    /// 设备标记的签名密钥。
    pub(crate) fn device_key(&self) -> Result<[u8; 32]> {
        let key: Option<Vec<u8>> = self
            .lock()
            .connection
            .query_row("SELECT device_key FROM owner", [], |row| row.get(0))
            .optional()?;
        let Some(key) = key else { bail!("认证库里还没有所有者凭据") };
        key.try_into().map_err(|_| anyhow::anyhow!("认证库里的设备标记密钥长度不对"))
    }

    /// 读出所有者凭据：哈希与凭据代数来自同一次读取。服务器不缓存哈希。
    pub(crate) fn credential(&self) -> Result<Option<Credential>> {
        Ok(self
            .lock()
            .connection
            .query_row("SELECT password_hash, generation FROM owner", [], |row| {
                Ok(Credential { password_hash: row.get(0)?, generation: row.get(1)? })
            })
            .optional()?)
    }

    /// 为核对过的 `credential` 建立新会话。凭据代数在读出之后变了（期间改了密码）时不建立，
    /// 返回 `None`：这次核对的是旧密码。顺带清掉已经过期的会话，表不会无限增长。
    pub(crate) fn create_session(
        &self,
        credential: &Credential,
        now: i64,
    ) -> Result<Option<SessionToken>> {
        let token = SessionToken::generate();
        let inner = self.lock();
        purge(&inner.connection, now)?;
        // 一条语句里再读一次凭据代数：与改密码的事务互斥，不会插入旧凭据下的会话。
        let inserted = inner.connection.execute(
            "INSERT INTO sessions (digest, created_at, last_used_at, generation)
             SELECT ?1, ?2, ?2, generation FROM owner WHERE generation = ?3",
            params![token.digest(), now, credential.generation],
        )?;
        Ok((inserted == 1).then_some(token))
    }

    /// 会话在 `now` 是否有效；有效时按需续期，无效时删除。
    ///
    /// - 待吊销的会话（此前删除失败）一律拒绝，并重试删除；
    /// - 判定失效即删除；删除失败照样拒绝，并记为待吊销；
    /// - 需要续期而写入失败时返回错误：这次使用没有记录，就不放行，空闲期限因此不会提前到来。
    pub(crate) fn validate(&self, token: &SessionToken, now: i64) -> Result<bool> {
        let digest = token.digest();
        let mut inner = self.lock();
        if inner.pending_revocations.contains(&digest) {
            if let Err(error) = inner.revoke(&digest) {
                warn!(%error, "仍无法删除待吊销的会话，继续拒绝它");
            }
            return Ok(false);
        }
        let Some(stored) = read_session(&inner.connection, &digest)? else {
            return Ok(false);
        };
        judge(&mut inner, &digest, stored, now)
    }

    /// 吊销一个会话（退出登录）。删除失败时返回错误，但本进程内不再接受这个会话。只有确实
    /// 存在（或已待吊销）的会话才会记为待吊销：伪造的令牌不占内存。
    pub(crate) fn revoke(&self, token: &SessionToken) -> Result<()> {
        let digest = token.digest();
        let mut inner = self.lock();
        if !inner.pending_revocations.contains(&digest)
            && read_session(&inner.connection, &digest)?.is_none()
        {
            return Ok(());
        }
        inner.revoke(&digest)
    }

    /// 删除按 `now` 已经失效的会话（空闲、绝对上限、不可能的记录、旧凭据下的会话）。服务器
    /// 启动时调用：待吊销集合不跨重启，过期的会话在这里落实删除；认证库不可写时启动失败。
    pub(crate) fn purge_expired(&self, now: i64) -> Result<usize> {
        purge(&self.lock().connection, now)
    }

    /// 改密码：在一个事务里换哈希、凭据代数加一、删除全部会话。还没有凭据时建立它。
    pub(crate) fn change_password(&self, password_hash: &str) -> Result<()> {
        let mut inner = self.lock();
        let transaction =
            inner.connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let updated = transaction.execute(
            "UPDATE owner SET password_hash = ?1, generation = generation + 1",
            [password_hash],
        )?;
        if updated == 0 {
            transaction.execute(
                "INSERT INTO owner (id, password_hash, generation, device_key) VALUES (1, ?1, 1, ?2)",
                params![password_hash, random_key()],
            )?;
        }
        transaction.execute("DELETE FROM sessions", [])?;
        transaction.commit()?;
        Ok(())
    }

    /// 吊销全部会话，保留凭据（恢复备份时用）。返回吊销的数目。
    pub(crate) fn revoke_all(&self) -> Result<usize> {
        Ok(self.lock().connection.execute("DELETE FROM sessions", [])?)
    }
}

/// 按读到的记录 `stored` 判定并执行：失效即删除（删除失败记为待吊销）；需要续期就写入，
/// 写不进就返回错误。续期没有更新任何行，说明记录在读出之后被别的连接改过（续期或删除）：
/// 重读一次，按同样的规则再判定，包括失效时的删除。
fn judge(inner: &mut Inner, digest: &[u8; 32], mut stored: Stored, now: i64) -> Result<bool> {
    for _ in 0..2 {
        match stored.verdict(now) {
            Verdict::Reject(rejection) => {
                if let Err(error) = inner.revoke(digest) {
                    warn!(?rejection, %error, "删除失效的会话失败，本进程内继续拒绝它");
                }
                return Ok(false);
            }
            Verdict::Valid { renew: false } => return Ok(true),
            Verdict::Valid { renew: true } => {
                if renew_session(&inner.connection, digest, now)? {
                    return Ok(true);
                }
                match read_session(&inner.connection, digest)? {
                    Some(current) => stored = current,
                    None => return Ok(false),
                }
            }
        }
    }
    bail!("会话记录在两次读取之后都被别的连接改动，无法记录这次使用")
}

/// 删除按 `now` 已经失效的会话，返回删除的数目。条件与 [`Stored::verdict`] 的拒绝条件一致
/// （时钟回拨除外：它只在会话被出示时判定）；没有凭据时，全部会话都不属于任何凭据。
fn purge(connection: &Connection, now: i64) -> Result<usize> {
    Ok(connection.execute(
        "DELETE FROM sessions
         WHERE created_at <= ?1 - ?2
            OR last_used_at <= ?1 - ?3
            OR created_at > last_used_at
            OR generation < 1
            OR NOT EXISTS (SELECT 1 FROM owner o WHERE o.generation = sessions.generation)",
        params![now, ABSOLUTE, IDLE + RECORD_GRANULARITY],
    )?)
}

/// 把 `path` 所在目录的改动（新建、删除）同步到磁盘。只在 Unix 上需要并可行。
fn sync_directory(path: &Path) -> Result<()> {
    #[cfg(unix)]
    if let Some(directory) = path.parent() {
        std::fs::File::open(directory)
            .and_then(|directory| directory.sync_all())
            .with_context(|| format!("无法同步目录 {}", directory.display()))?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

fn read_session(connection: &Connection, digest: &[u8; 32]) -> Result<Option<Stored>> {
    Ok(connection
        .query_row(
            "SELECT s.created_at, s.last_used_at, s.generation, o.generation
             FROM sessions s JOIN owner o WHERE s.digest = ?1",
            [&digest[..]],
            |row| {
                Ok(Stored {
                    created_at: row.get(0)?,
                    last_used_at: row.get(1)?,
                    generation: row.get(2)?,
                    current_generation: row.get(3)?,
                })
            },
        )
        .optional()?)
}

fn delete_session(connection: &Connection, digest: &[u8; 32]) -> Result<()> {
    connection.execute("DELETE FROM sessions WHERE digest = ?1", [&digest[..]])?;
    Ok(())
}

/// 记录使用时间，返回是否写入。只更新仍存在的行（被删除的会话不会复活），只在距上次记录满
/// 一个粒度时写，新值总比旧值晚（从不往回改）：并发的续期里只有一个真正写入。
fn renew_session(connection: &Connection, digest: &[u8; 32], now: i64) -> Result<bool> {
    let updated = connection.execute(
        "UPDATE sessions SET last_used_at = ?2 WHERE digest = ?1 AND last_used_at <= ?2 - ?3",
        params![&digest[..], now, RECORD_GRANULARITY],
    )?;
    Ok(updated == 1)
}

fn migrate(connection: &mut Connection) -> Result<()> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let version: i64 = transaction.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    match version {
        0 => {
            transaction.execute_batch(SCHEMA)?;
            transaction.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        }
        SCHEMA_VERSION => {}
        newer => bail!("认证库的结构版本 {newer} 比本程序支持的 {SCHEMA_VERSION} 新"),
    }
    transaction.commit()?;
    Ok(())
}

fn random_key() -> [u8; 32] {
    let mut key = [0_u8; 32];
    OsRng.fill_bytes(&mut key);
    key
}

/// 准备数据目录与认证库文件，返回文件路径。
///
/// 文件以 0600 新建；SQLite 新建的 `-wal`、`-shm` 沿用它的权限。已存在的主库与伴随文件
/// （`-wal`、`-shm`、`-journal`，例如恢复或异常退出留下的）都必须是普通文件；Unix 上还要属于
/// 当前用户，且其他用户无权访问：已有的伴随文件不会因为主库是 0600 就被改正。
fn prepare_file(data_dir: &Path) -> Result<PathBuf> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(data_dir).with_context(|| format!("无法创建数据目录 {}", data_dir.display()))?;
    #[cfg(unix)]
    {
        let metadata = std::fs::metadata(data_dir)
            .with_context(|| format!("无法读取数据目录 {}", data_dir.display()))?;
        check_private(data_dir, &metadata, 0o022)?;
    }

    let path = data_dir.join(AUTH_DB);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    match options.open(&path) {
        Ok(_) => {}
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {
            let metadata = std::fs::symlink_metadata(&path)
                .with_context(|| format!("无法读取认证库 {}", path.display()))?;
            if !metadata.file_type().is_file() {
                bail!("认证库 {} 不是普通文件", path.display());
            }
            #[cfg(unix)]
            check_private(&path, &metadata, 0o077)?;
        }
        Err(error) => {
            return Err(error).with_context(|| format!("无法创建认证库 {}", path.display()));
        }
    }
    for suffix in ["-wal", "-shm", "-journal"] {
        let companion = data_dir.join(format!("{AUTH_DB}{suffix}"));
        match std::fs::symlink_metadata(&companion) {
            Ok(metadata) => {
                if !metadata.file_type().is_file() {
                    bail!("认证库的伴随文件 {} 不是普通文件", companion.display());
                }
                #[cfg(unix)]
                check_private(&companion, &metadata, 0o077)?;
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error).with_context(|| format!("无法读取 {}", companion.display()));
            }
        }
    }
    Ok(path)
}

/// `path` 属于当前用户，且权限里没有 `forbidden` 中的位。
#[cfg(unix)]
fn check_private(path: &Path, metadata: &std::fs::Metadata, forbidden: u32) -> Result<()> {
    use std::os::unix::fs::MetadataExt;
    // SAFETY: geteuid 没有前置条件，也不会失败。
    let user = unsafe { libc::geteuid() };
    if metadata.uid() != user {
        bail!(
            "{} 属于用户 {}，当前用户是 {user}：请以数据目录的所有者运行",
            path.display(),
            metadata.uid()
        );
    }
    let mode = metadata.mode() & 0o777;
    if mode & forbidden != 0 {
        bail!("{} 的权限 {mode:o} 过宽：其他用户不能访问它", path.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const T0: i64 = 1_800_000_000;

    use crate::auth::test_support::ScratchDir as Scratch;

    fn open(scratch: &Scratch) -> AuthStore {
        let store = AuthStore::open(scratch.path()).unwrap();
        store.initialize(|| Ok("hash-v1".to_owned())).unwrap();
        store
    }

    fn login(store: &AuthStore, now: i64) -> SessionToken {
        let credential = store.credential().unwrap().unwrap();
        store.create_session(&credential, now).unwrap().expect("unchanged credential")
    }

    fn sessions(store: &AuthStore) -> i64 {
        store
            .lock()
            .connection
            .query_row("SELECT count(*) FROM sessions", [], |row| row.get(0))
            .unwrap()
    }

    fn last_used(store: &AuthStore, token: &SessionToken) -> i64 {
        read_session(&store.lock().connection, &token.digest()).unwrap().unwrap().last_used_at
    }

    /// 只在创建时用过一次的会话：第 29 天、第 30 天有效，30 天加 1 小时整前一秒仍有效，
    /// 整点时过期并被删除。
    #[test]
    fn an_unused_session_expires_thirty_days_and_one_granularity_after_its_last_record() {
        let scratch = Scratch::new("idle");
        let store = open(&scratch);
        for (offset, valid) in [
            (29 * DAY, true),
            (30 * DAY, true),
            (30 * DAY + HOUR - 1, true),
            (30 * DAY + HOUR, false),
            (31 * DAY, false),
        ] {
            let token = login(&store, T0);
            assert_eq!(store.validate(&token, T0 + offset).unwrap(), valid, "{offset}");
        }
        // 每次过期都删掉了自己的行：只剩三个有效的。
        assert_eq!(sessions(&store), 3);
    }

    /// 不满一个粒度的使用不写库，空闲期限不动；满一个粒度（整点）时写入，期限随之后移。
    #[test]
    fn use_is_recorded_once_per_granularity_and_moves_the_idle_deadline() {
        let scratch = Scratch::new("renew");
        let store = open(&scratch);

        let unrecorded = login(&store, T0);
        assert!(store.validate(&unrecorded, T0 + HOUR - 1).unwrap());
        assert_eq!(last_used(&store, &unrecorded), T0, "an hour has not passed");
        assert!(!store.validate(&unrecorded, T0 + 30 * DAY + HOUR).unwrap());

        let recorded = login(&store, T0);
        assert!(store.validate(&recorded, T0 + HOUR).unwrap());
        assert_eq!(last_used(&store, &recorded), T0 + HOUR);
        assert!(store.validate(&recorded, T0 + HOUR + 30 * DAY + HOUR - 1).unwrap());

        let lapsed = login(&store, T0);
        assert!(store.validate(&lapsed, T0 + HOUR).unwrap());
        assert!(!store.validate(&lapsed, T0 + HOUR + 30 * DAY + HOUR).unwrap());
    }

    /// 续期语句自身的保证，不依赖调用方先判断粒度（例如另一个进程打开了同一个库）：不满一个
    /// 粒度不写，从不往回改。
    #[test]
    fn the_renewal_statement_writes_hourly_and_never_backwards() {
        let scratch = Scratch::new("renew-statement");
        let store = open(&scratch);
        let token = login(&store, T0);
        let renew = |now| renew_session(&store.lock().connection, &token.digest(), now).unwrap();

        renew(T0 + HOUR - 1);
        assert_eq!(last_used(&store, &token), T0, "less than a granularity later");
        renew(T0 + 2 * HOUR);
        assert_eq!(last_used(&store, &token), T0 + 2 * HOUR);
        renew(T0 + HOUR + 1);
        assert_eq!(last_used(&store, &token), T0 + 2 * HOUR, "never moved backwards");
        renew(T0 + 3 * HOUR - 1);
        assert_eq!(last_used(&store, &token), T0 + 2 * HOUR);
        renew(T0 + 3 * HOUR);
        assert_eq!(last_used(&store, &token), T0 + 3 * HOUR);
    }

    /// 天天使用的会话也在创建后 180 天整过期；之后原样重放这个 cookie 仍被拒绝。
    #[test]
    fn a_session_in_daily_use_still_ends_at_the_absolute_limit() {
        let scratch = Scratch::new("absolute");
        let store = open(&scratch);
        let token = login(&store, T0);
        for day in 1..180 {
            assert!(store.validate(&token, T0 + day * DAY).unwrap(), "day {day}");
        }
        assert!(store.validate(&token, T0 + ABSOLUTE - 1).unwrap());
        assert!(!store.validate(&token, T0 + ABSOLUTE).unwrap());
        assert_eq!(sessions(&store), 0);
        let replayed = SessionToken::parse(token.as_str()).unwrap();
        assert!(!store.validate(&replayed, T0 + ABSOLUTE + DAY).unwrap());
        assert!(!store.validate(&replayed, T0 + DAY).unwrap(), "nor with the clock set back");
    }

    /// 时钟比最后记录的使用时间早 5 分钟以内照常有效、不往回改；早得更多时拒绝并删除，
    /// 时钟恢复后也不复活。
    #[test]
    fn a_clock_set_back_more_than_the_tolerance_rejects_and_deletes() {
        let scratch = Scratch::new("clock");
        let store = open(&scratch);
        let token = login(&store, T0);
        let recorded = T0 + 2 * HOUR;
        assert!(store.validate(&token, recorded).unwrap());

        assert!(store.validate(&token, recorded - CLOCK_TOLERANCE).unwrap());
        assert_eq!(last_used(&store, &token), recorded, "never moved backwards");
        assert!(!store.validate(&token, recorded - CLOCK_TOLERANCE - 1).unwrap());
        assert_eq!(sessions(&store), 0);
        assert!(!store.validate(&token, recorded + 10).unwrap());
    }

    /// 过期即删除：之后把时钟拨回过期之前，会话也不会复活。
    #[test]
    fn an_expired_session_stays_invalid_when_the_clock_goes_back() {
        let scratch = Scratch::new("expired");
        let store = open(&scratch);
        let token = login(&store, T0);
        assert!(!store.validate(&token, T0 + 31 * DAY).unwrap());
        assert!(!store.validate(&token, T0 + 1).unwrap());
    }

    #[test]
    fn sessions_survive_a_restart() {
        let scratch = Scratch::new("restart");
        let token = login(&open(&scratch), T0);
        let reopened = AuthStore::open(scratch.path()).unwrap();
        assert!(reopened.validate(&token, T0 + DAY).unwrap());
    }

    /// 退出与续期的两种先后顺序：先退出，续期不会重建会话；先续期，退出照样吊销。
    #[test]
    fn logout_and_renewal_in_either_order() {
        let scratch = Scratch::new("logout");
        let store = open(&scratch);

        let token = login(&store, T0);
        store.revoke(&token).unwrap();
        renew_session(&store.lock().connection, &token.digest(), T0 + 2 * HOUR).unwrap();
        assert_eq!(sessions(&store), 0, "renewal never recreates a session");
        assert!(!store.validate(&token, T0 + 2 * HOUR).unwrap());

        let token = login(&store, T0);
        assert!(store.validate(&token, T0 + 2 * HOUR).unwrap());
        store.revoke(&token).unwrap();
        assert!(!store.validate(&token, T0 + 2 * HOUR + 1).unwrap());
    }

    /// 登录与改密码的两种先后顺序：凭据读出后改了密码，会话不建立；会话建立后改密码，它被吊销。
    #[test]
    fn login_and_password_change_in_either_order() {
        let scratch = Scratch::new("passwd");
        let store = open(&scratch);

        let credential = store.credential().unwrap().unwrap();
        store.change_password("hash-v2").unwrap();
        assert!(store.create_session(&credential, T0).unwrap().is_none());
        assert_eq!(sessions(&store), 0);

        let token = login(&store, T0);
        assert_eq!(store.credential().unwrap().unwrap().password_hash, "hash-v2");
        store.change_password("hash-v3").unwrap();
        assert!(!store.validate(&token, T0 + 1).unwrap());
    }

    /// 凭据代数与会话不一致的行（改密码的删除之外的第二道保险）被拒绝并删除。
    #[test]
    fn a_session_from_an_older_credential_is_rejected() {
        let scratch = Scratch::new("generation");
        let store = open(&scratch);
        let token = login(&store, T0);
        store
            .lock()
            .connection
            .execute("UPDATE owner SET generation = generation + 1", [])
            .unwrap();
        assert!(!store.validate(&token, T0 + 1).unwrap());
        assert_eq!(sessions(&store), 0);
    }

    #[test]
    fn revoke_all_keeps_the_credential() {
        let scratch = Scratch::new("revoke-all");
        let store = open(&scratch);
        let tokens = [login(&store, T0), login(&store, T0)];
        let generation = store.credential().unwrap().unwrap().generation;
        assert_eq!(store.revoke_all().unwrap(), 2);
        for token in &tokens {
            assert!(!store.validate(token, T0 + 1).unwrap());
        }
        let credential = store.credential().unwrap().unwrap();
        assert_eq!(
            (credential.password_hash.as_str(), credential.generation),
            ("hash-v1", generation)
        );
        login(&store, T0 + 2);
    }

    /// 新登录顺带清掉已经过期、再也不会出示的会话。
    #[test]
    fn a_login_purges_expired_sessions() {
        let scratch = Scratch::new("purge");
        let store = open(&scratch);
        login(&store, T0);
        let recent = login(&store, T0 + DAY);
        login(&store, T0 + IDLE + HOUR);
        assert_eq!(sessions(&store), 2, "only the first one passed its idle limit");
        assert!(store.validate(&recent, T0 + IDLE + HOUR).unwrap());

        let scratch = Scratch::new("purge-absolute");
        let store = open(&scratch);
        let busy = login(&store, T0);
        for period in 1..=6 {
            assert!(store.validate(&busy, T0 + period * 29 * DAY).unwrap());
        }
        login(&store, T0 + ABSOLUTE);
        assert_eq!(sessions(&store), 1, "a session in use still ends at the absolute limit");
    }

    #[test]
    fn the_credential_is_seeded_once() {
        let scratch = Scratch::new("seed");
        let store = AuthStore::open(scratch.path()).unwrap();
        assert!(!store.has_owner().unwrap());
        assert!(store.initialize(|| Ok("first".to_owned())).unwrap());
        let key = store.device_key().unwrap();
        assert!(!store.initialize(|| panic!("seeded already")).unwrap());
        store.change_password("second").unwrap();
        assert_eq!(store.device_key().unwrap(), key, "a password change keeps the device key");
    }

    /// v0.1 的 HMAC 会话 cookie 与形状正确但库里没有的令牌都被拒绝。
    #[test]
    fn unknown_and_legacy_cookies_are_rejected() {
        use hmac::{Hmac, Mac};
        let scratch = Scratch::new("legacy");
        let store = open(&scratch);
        let payload = URL_SAFE_NO_PAD.encode("demo:9999999999");
        let mut mac = Hmac::<Sha256>::new_from_slice(&[7; 32]).unwrap();
        mac.update(b"demo:9999999999");
        let legacy = format!("{payload}.{}", URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes()));
        assert!(SessionToken::parse(&legacy).is_none());
        assert!(!store.validate(&SessionToken::generate(), T0).unwrap());
    }

    #[test]
    fn the_csrf_token_is_fixed_per_session_and_reveals_nothing() {
        let token = SessionToken::generate();
        let again = SessionToken::parse(token.as_str()).unwrap();
        assert_eq!(token.csrf(), again.csrf());
        assert_ne!(token.csrf(), SessionToken::generate().csrf());
        assert_ne!(token.csrf(), token.as_str());
        assert_ne!(token.csrf(), URL_SAFE_NO_PAD.encode(token.digest()));
    }

    #[cfg(unix)]
    #[test]
    fn the_database_is_private_to_its_owner() {
        use std::os::unix::fs::PermissionsExt;
        let scratch = Scratch::new("private");
        open(&scratch);
        let path = scratch.path().join(AUTH_DB);
        assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);

        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o640)).unwrap();
        assert!(AuthStore::open(scratch.path()).is_err(), "readable by the group");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        std::fs::set_permissions(scratch.path(), std::fs::Permissions::from_mode(0o777)).unwrap();
        assert!(AuthStore::open(scratch.path()).is_err(), "a directory others can write");
        std::fs::set_permissions(scratch.path(), std::fs::Permissions::from_mode(0o700)).unwrap();

        let linked = Scratch::new("linked");
        std::os::unix::fs::symlink(&path, linked.path().join(AUTH_DB)).unwrap();
        assert!(AuthStore::open(linked.path()).is_err(), "a symbolic link");
    }

    /// Makes every write of `store` fail (SQLITE_READONLY) while reads keep working.
    fn writes_fail(store: &AuthStore, fail: bool) {
        store.lock().connection.pragma_update(None, "query_only", fail).unwrap();
    }

    /// 判定失效、删除却失败的会话不会因为时钟恢复而复活：本进程一直拒绝它，再次出示时重试
    /// 删除（评审 B1）。时钟回拨与空闲过期两种拒绝都试。
    #[test]
    fn a_session_whose_deletion_failed_stays_rejected() {
        let scratch = Scratch::new("pending");
        let store = open(&scratch);

        let rolled_back = login(&store, T0);
        let recorded = T0 + 2 * HOUR;
        assert!(store.validate(&rolled_back, recorded).unwrap());
        writes_fail(&store, true);
        assert!(!store.validate(&rolled_back, recorded - CLOCK_TOLERANCE - 1).unwrap());
        assert!(!store.validate(&rolled_back, recorded + 10).unwrap(), "the clock came back");
        assert_eq!(sessions(&store), 1, "the row is still there");

        let idle = login_with_writes(&store, T0);
        assert!(!store.validate(&idle, T0 + IDLE + HOUR).unwrap());
        assert!(!store.validate(&idle, T0 + 1).unwrap(), "the clock went back before expiry");

        writes_fail(&store, false);
        assert!(!store.validate(&rolled_back, recorded + 20).unwrap());
        assert!(!store.validate(&idle, T0 + 2).unwrap());
        assert_eq!(sessions(&store), 0, "retried deletions succeeded");
        assert!(store.lock().pending_revocations.is_empty());
    }

    /// Logs in while writes fail for everything else: the login itself must be written.
    fn login_with_writes(store: &AuthStore, now: i64) -> SessionToken {
        writes_fail(store, false);
        let token = login(store, now);
        writes_fail(store, true);
        token
    }

    /// 退出登录时删除失败：报错，但本进程内这个会话已不再被接受。
    #[test]
    fn a_failed_logout_still_ends_the_session_here() {
        let scratch = Scratch::new("failed-logout");
        let store = open(&scratch);
        let token = login(&store, T0);
        writes_fail(&store, true);
        assert!(store.revoke(&token).is_err());
        assert!(!store.validate(&token, T0 + 1).unwrap());
        writes_fail(&store, false);
        assert!(!store.validate(&token, T0 + 2).unwrap());
        assert_eq!(sessions(&store), 0);
    }

    /// 需要续期而写不进时不放行（评审 S1）；不需要续期时只读即可。
    #[test]
    fn a_use_that_cannot_be_recorded_is_refused() {
        let scratch = Scratch::new("failed-renewal");
        let store = open(&scratch);
        let token = login(&store, T0);
        writes_fail(&store, true);
        assert!(store.validate(&token, T0 + HOUR - 1).unwrap(), "no renewal needed");
        assert!(store.validate(&token, T0 + HOUR).is_err(), "a renewal that fails");
        assert_eq!(last_used(&store, &token), T0);
        writes_fail(&store, false);
        assert!(store.validate(&token, T0 + HOUR).unwrap());
        assert_eq!(last_used(&store, &token), T0 + HOUR);
    }

    /// 另一个进程续过期（这里不再写）或删掉了会话（这里随即拒绝）。
    #[test]
    fn renewal_and_deletion_by_another_process_are_seen() {
        let scratch = Scratch::new("renewed-elsewhere");
        let store = open(&scratch);
        let token = login(&store, T0);
        let other = AuthStore::open(scratch.path()).unwrap();
        assert!(other.validate(&token, T0 + 2 * HOUR).unwrap());
        // This store read the row before the other one renewed it: replay its decision.
        assert!(!renew_session(&store.lock().connection, &token.digest(), T0 + 2 * HOUR).unwrap());
        assert!(store.validate(&token, T0 + 2 * HOUR).unwrap());
        other.revoke(&token).unwrap();
        assert!(!store.validate(&token, T0 + 4 * HOUR).unwrap());
    }

    /// 待吊销集合不跨重启：启动时的清理落实过期会话的删除，旧凭据下的与不可能的记录也一并删除。
    #[test]
    fn the_startup_purge_deletes_what_a_restart_forgot() {
        let scratch = Scratch::new("restart-purge");
        let store = open(&scratch);
        let expired = login(&store, T0);
        let current = login(&store, T0 + 20 * DAY);
        let impossible = login(&store, T0 + 20 * DAY);
        writes_fail(&store, true);
        assert!(!store.validate(&expired, T0 + IDLE + HOUR).unwrap());
        writes_fail(&store, false);
        store
            .lock()
            .connection
            .execute(
                "UPDATE sessions SET created_at = ?1 WHERE digest = ?2",
                params![i64::MAX, &impossible.digest()[..]],
            )
            .unwrap();
        drop(store);

        let restarted = AuthStore::open(scratch.path()).unwrap();
        assert_eq!(restarted.purge_expired(T0 + IDLE + HOUR).unwrap(), 2);
        assert!(!restarted.validate(&expired, T0 + 1).unwrap());
        assert!(restarted.validate(&current, T0 + IDLE + HOUR).unwrap());
        assert_eq!(sessions(&restarted), 1);
    }

    /// 第二轮评审 B-1：判过期、删除失败，进程异常退出（待吊销集合随之丢失），重启时时钟已经
    /// 回到过期之前。运行标记在出错之前就已落盘，下次启动吊销全部会话，旧会话不会复活。
    #[test]
    fn an_unclean_exit_revokes_every_session_at_the_next_start() {
        let scratch = Scratch::new("unclean-exit");
        let store = open(&scratch);
        assert!(!store.begin_run().unwrap(), "a first start");
        let expired = login(&store, T0);
        writes_fail(&store, true);
        assert!(!store.validate(&expired, T0 + IDLE + HOUR).unwrap());
        drop(store); // no end_run: the process died

        let restarted = AuthStore::open(scratch.path()).unwrap();
        assert!(restarted.begin_run().unwrap(), "the marker was still there");
        assert_eq!(restarted.purge_expired(T0 + IDLE + HOUR - 60).unwrap(), 0);
        assert!(!restarted.validate(&expired, T0 + IDLE + HOUR - 60).unwrap());
        assert_eq!(sessions(&restarted), 0);
    }

    /// 正常停止且没有待吊销的会话：移除运行标记，下次启动保留全部会话。
    #[test]
    fn a_clean_exit_keeps_sessions() {
        let scratch = Scratch::new("clean-exit");
        let store = open(&scratch);
        assert!(!store.begin_run().unwrap());
        assert!(scratch.path().join(RUN_MARKER).is_file());
        let token = login(&store, T0);
        assert!(store.end_run().unwrap());
        assert!(!scratch.path().join(RUN_MARKER).exists());
        drop(store);

        let restarted = AuthStore::open(scratch.path()).unwrap();
        assert!(!restarted.begin_run().unwrap());
        assert!(restarted.validate(&token, T0 + 1).unwrap());
    }

    /// 停止时先重试待吊销的删除：成功就正常结束；仍失败就保留标记，下次启动吊销全部会话。
    #[test]
    fn an_exit_with_unresolved_revocations_keeps_the_marker() {
        let scratch = Scratch::new("unresolved-exit");
        let store = open(&scratch);
        store.begin_run().unwrap();
        let kept = login(&store, T0);
        let expired = login(&store, T0 - IDLE);
        writes_fail(&store, true);
        assert!(!store.validate(&expired, T0 + HOUR).unwrap());
        assert!(!store.end_run().unwrap(), "the deletion still fails");
        assert!(scratch.path().join(RUN_MARKER).is_file());
        writes_fail(&store, false);
        drop(store);

        let restarted = AuthStore::open(scratch.path()).unwrap();
        assert!(restarted.begin_run().unwrap());
        assert!(!restarted.validate(&kept, T0 + 1).unwrap(), "every session was revoked");

        let scratch = Scratch::new("resolved-exit");
        let store = open(&scratch);
        store.begin_run().unwrap();
        let expired = login(&store, T0 - IDLE);
        writes_fail(&store, true);
        assert!(!store.validate(&expired, T0 + HOUR).unwrap());
        writes_fail(&store, false);
        assert!(store.end_run().unwrap(), "the retry at exit succeeded");
        assert_eq!(sessions(&store), 0);
    }

    /// 第二轮评审 S-1：读出记录之后，别的连接把使用时间改到了更晚（这里晚得构成时钟回拨）。
    /// 续期没有更新任何行，重读后按同样的规则判定：拒绝并删除，而不只是拒绝。
    #[test]
    fn a_renewal_that_updates_nothing_judges_the_current_row() {
        let scratch = Scratch::new("judge-again");
        let store = open(&scratch);
        let other = AuthStore::open(scratch.path()).unwrap();
        let now = T0 + 2 * HOUR;
        let digest = login(&store, T0).digest();
        let stale = read_session(&store.lock().connection, &digest).unwrap().unwrap();
        other
            .lock()
            .connection
            .execute(
                "UPDATE sessions SET last_used_at = ?1 WHERE digest = ?2",
                params![now + CLOCK_TOLERANCE + 1, &digest[..]],
            )
            .unwrap();
        assert!(!judge(&mut store.lock(), &digest, stale, now).unwrap());
        assert_eq!(sessions(&store), 0, "rejected and deleted");

        let digest = login(&store, T0).digest();
        let stale = read_session(&store.lock().connection, &digest).unwrap().unwrap();
        renew_session(&other.lock().connection, &digest, now).unwrap();
        assert!(judge(&mut store.lock(), &digest, stale, now).unwrap(), "renewed elsewhere");
    }

    /// 第二轮评审 B-2：退出一个并不存在的会话（伪造的令牌）什么也不记，即使写入失败。
    #[test]
    fn logging_out_an_unknown_session_records_nothing() {
        let scratch = Scratch::new("unknown-logout");
        let store = open(&scratch);
        writes_fail(&store, true);
        store.revoke(&SessionToken::generate()).unwrap();
        assert!(store.lock().pending_revocations.is_empty());
    }

    /// 第二轮评审 N-1：没有凭据时留下的会话不属于任何凭据：清理时删除，建立凭据时一并清除。
    #[test]
    fn sessions_without_an_owner_are_removed() {
        let scratch = Scratch::new("orphans");
        let store = open(&scratch);
        login(&store, T0);
        store.lock().connection.execute("DELETE FROM owner", []).unwrap();
        assert_eq!(store.purge_expired(T0 + 1).unwrap(), 1);

        let scratch = Scratch::new("orphans-initialize");
        let store = open(&scratch);
        let orphan = login(&store, T0);
        store.lock().connection.execute("DELETE FROM owner", []).unwrap();
        assert!(store.initialize(|| Ok("hash-v2".to_owned())).unwrap());
        assert!(!store.validate(&orphan, T0 + 1).unwrap());
        assert_eq!(sessions(&store), 0);
    }

    /// 不可能的记录不按它计算期限（评审 N1）。
    #[test]
    fn an_impossible_record_is_rejected() {
        let scratch = Scratch::new("impossible");
        let store = open(&scratch);
        for (column, value) in [("created_at", i64::MAX), ("generation", 0)] {
            let token = login(&store, T0);
            store
                .lock()
                .connection
                .execute(
                    &format!("UPDATE sessions SET {column} = ?1 WHERE digest = ?2"),
                    params![value, &token.digest()[..]],
                )
                .unwrap();
            assert!(!store.validate(&token, T0).unwrap(), "{column}");
        }
        assert_eq!(sessions(&store), 0);
    }

    /// 已有的伴随文件（恢复或异常退出留下的）同样检查：权限过宽或不是普通文件时拒绝打开
    /// （评审 S4）。
    #[cfg(unix)]
    #[test]
    fn existing_companion_files_are_checked() {
        use std::os::unix::fs::PermissionsExt;
        for suffix in ["-wal", "-shm", "-journal"] {
            let scratch = Scratch::new("companion");
            drop(open(&scratch));
            let companion = scratch.path().join(format!("{AUTH_DB}{suffix}"));
            std::fs::write(&companion, b"leftover pages").unwrap();
            std::fs::set_permissions(&companion, std::fs::Permissions::from_mode(0o644)).unwrap();
            assert!(AuthStore::open(scratch.path()).is_err(), "{suffix} readable by others");
            std::fs::set_permissions(&companion, std::fs::Permissions::from_mode(0o600)).unwrap();
            if suffix == "-journal" {
                std::fs::remove_file(&companion).unwrap();
                std::os::unix::fs::symlink("/tmp", &companion).unwrap();
                assert!(AuthStore::open(scratch.path()).is_err(), "a linked {suffix}");
            }
        }
    }
}
