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
    CredentialChanged,
    Absolute,
    Idle,
}

impl Stored {
    /// 会话在 `now` 是否有效。
    ///
    /// 不变式：库里的 `last_used_at` 不晚于最后一次真实使用，且真实使用 `t` 满足
    /// `t < last_used_at + RECORD_GRANULARITY`（再晚的使用会触发续期）。因此空闲期限
    /// `last_used_at + IDLE + RECORD_GRANULARITY` 晚于 `t + IDLE`（不提前过期），且不晚于
    /// `t + IDLE + RECORD_GRANULARITY`（最多晚一个粒度）。差值用饱和减法，库里的异常值不会溢出。
    fn verdict(&self, now: i64) -> Verdict {
        if self.last_used_at.saturating_sub(now) > CLOCK_TOLERANCE {
            return Verdict::Reject(Rejection::ClockSetBack);
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
    connection: Mutex<Connection>,
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
        Ok(Self { connection: Mutex::new(connection) })
    }

    fn connection(&self) -> MutexGuard<'_, Connection> {
        // 持锁时 panic 的语句已随事务回滚，连接本身仍可用。
        self.connection.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// 是否已有所有者凭据。
    pub(crate) fn has_owner(&self) -> Result<bool> {
        let found: Option<i64> =
            self.connection().query_row("SELECT 1 FROM owner", [], |row| row.get(0)).optional()?;
        Ok(found.is_some())
    }

    /// 还没有所有者凭据时，用 `seed` 给出的哈希建立它（凭据代数 1，新的设备标记密钥），返回
    /// 是否由这次调用建立。`seed` 只在确实没有凭据时调用；同时有别的进程建立时，先到者为准。
    pub(crate) fn initialize(&self, seed: impl FnOnce() -> Result<String>) -> Result<bool> {
        if self.has_owner()? {
            return Ok(false);
        }
        let password_hash = seed()?;
        let inserted = self.connection().execute(
            "INSERT INTO owner (id, password_hash, generation, device_key) VALUES (1, ?1, 1, ?2)
             ON CONFLICT (id) DO NOTHING",
            params![password_hash, random_key()],
        )?;
        Ok(inserted == 1)
    }

    /// 设备标记的签名密钥。
    pub(crate) fn device_key(&self) -> Result<[u8; 32]> {
        let key: Option<Vec<u8>> = self
            .connection()
            .query_row("SELECT device_key FROM owner", [], |row| row.get(0))
            .optional()?;
        let Some(key) = key else { bail!("认证库里还没有所有者凭据") };
        key.try_into().map_err(|_| anyhow::anyhow!("认证库里的设备标记密钥长度不对"))
    }

    /// 读出所有者凭据：哈希与凭据代数来自同一次读取。服务器不缓存哈希。
    pub(crate) fn credential(&self) -> Result<Option<Credential>> {
        Ok(self
            .connection()
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
        let connection = self.connection();
        connection.execute(
            "DELETE FROM sessions WHERE created_at <= ?1 - ?2 OR last_used_at <= ?1 - ?3",
            params![now, ABSOLUTE, IDLE + RECORD_GRANULARITY],
        )?;
        // 一条语句里再读一次凭据代数：与改密码的事务互斥，不会插入旧凭据下的会话。
        let inserted = connection.execute(
            "INSERT INTO sessions (digest, created_at, last_used_at, generation)
             SELECT ?1, ?2, ?2, generation FROM owner WHERE generation = ?3",
            params![token.digest(), now, credential.generation],
        )?;
        Ok((inserted == 1).then_some(token))
    }

    /// 会话在 `now` 是否有效；有效时按需续期，无效时删除。
    ///
    /// 删除失败也照样拒绝；续期失败只记日志，会话仍有效（它已被判定有效，续期只影响空闲期限）。
    pub(crate) fn validate(&self, token: &SessionToken, now: i64) -> Result<bool> {
        let digest = token.digest();
        let connection = self.connection();
        let Some(stored) = read_session(&connection, &digest)? else {
            return Ok(false);
        };
        match stored.verdict(now) {
            Verdict::Reject(rejection) => {
                if let Err(error) = delete_session(&connection, &digest) {
                    warn!(?rejection, %error, "删除被拒绝的会话失败");
                }
                Ok(false)
            }
            Verdict::Valid { renew } => {
                if renew && let Err(error) = renew_session(&connection, &digest, now) {
                    warn!(%error, "记录会话的使用时间失败");
                }
                Ok(true)
            }
        }
    }

    /// 吊销一个会话（退出登录）。
    pub(crate) fn revoke(&self, token: &SessionToken) -> Result<()> {
        delete_session(&self.connection(), &token.digest())
    }

    /// 改密码：在一个事务里换哈希、凭据代数加一、删除全部会话。还没有凭据时建立它。
    pub(crate) fn change_password(&self, password_hash: &str) -> Result<()> {
        let mut connection = self.connection();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
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
        Ok(self.connection().execute("DELETE FROM sessions", [])?)
    }
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

/// 记录使用时间。只更新仍存在的行（被删除的会话不会复活），只在距上次记录满一个粒度时写，
/// 新值总比旧值晚（从不往回改）：并发的续期里只有一个真正写入。
fn renew_session(connection: &Connection, digest: &[u8; 32], now: i64) -> Result<()> {
    connection.execute(
        "UPDATE sessions SET last_used_at = ?2 WHERE digest = ?1 AND last_used_at <= ?2 - ?3",
        params![&digest[..], now, RECORD_GRANULARITY],
    )?;
    Ok(())
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

/// 准备数据目录与认证库文件，返回文件路径。文件以 0600 新建（SQLite 的 `-wal`、`-shm` 沿用
/// 它的权限）；已存在时必须是普通文件，Unix 上还要属于当前用户且其他用户无权访问。
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
        store.connection().query_row("SELECT count(*) FROM sessions", [], |row| row.get(0)).unwrap()
    }

    fn last_used(store: &AuthStore, token: &SessionToken) -> i64 {
        read_session(&store.connection(), &token.digest()).unwrap().unwrap().last_used_at
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
        renew_session(&store.connection(), &token.digest(), T0 + 2 * HOUR).unwrap();
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
        store.connection().execute("UPDATE owner SET generation = generation + 1", []).unwrap();
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
}
