pub mod app_state_repository;
pub mod entry_repository;
pub mod feed_repository;
pub mod settings_repository;
pub mod sqlite_native;
pub mod storage_backend;

use std::str::FromStr;
use std::time::Duration;

use sqlx::{
    Pool, Sqlite,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
};

pub type SqlitePool = Pool<Sqlite>;

pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations");
pub static CONTENT_MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations_content");

/// 写锁等待上限。刷新可能是并发的（见 `rssr_application::DEFAULT_REFRESH_CONCURRENCY`），
/// 同一时刻只有一个写者能拿到 SQLite 的写锁，其余写者在这个时限内排队而不是立刻失败。
const SQLITE_BUSY_TIMEOUT: Duration = Duration::from_secs(30);

/// 给文件型 SQLite 连接补上 WAL 与 busy_timeout。
///
/// sqlx 的默认 journal 模式是 `delete`（回滚日志），在这种模式下写者会阻塞读者，
/// 并发刷新时 UI 查询会被写事务卡住；写者之间也只能靠 busy_timeout 干等。
/// WAL 让读者不再被写者阻塞，是「后台刷新 + 前台阅读」这种形态该用的模式。
///
/// 内存库不支持 WAL（`PRAGMA journal_mode=WAL` 在 `:memory:` 上不会生效），因此跳过。
fn apply_sqlite_tuning(options: SqliteConnectOptions, is_memory: bool) -> SqliteConnectOptions {
    let options = options.busy_timeout(SQLITE_BUSY_TIMEOUT);
    if is_memory { options } else { options.journal_mode(SqliteJournalMode::Wal) }
}

pub async fn create_sqlite_pool(database_url: &str) -> anyhow::Result<SqlitePool> {
    let is_memory = is_memory_database(database_url);
    let options = apply_sqlite_tuning(SqliteConnectOptions::from_str(database_url)?, is_memory);

    let pool = SqlitePoolOptions::new()
        .max_connections(default_sqlite_max_connections(database_url))
        .connect_with(options)
        .await?;

    Ok(pool)
}

pub(crate) fn connect_options_for_path(path: &std::path::Path) -> SqliteConnectOptions {
    apply_sqlite_tuning(SqliteConnectOptions::new().filename(path).create_if_missing(true), false)
}

/// 判断是不是内存库。
///
/// 这个判断同时决定池大小、是否发 WAL pragma，因此几种写法都要认出来：
/// `sqlite::memory:`、`sqlite://:memory:`、裸 `:memory:`，以及带 `mode=memory` 的 URI。
pub(crate) fn is_memory_database(database_url: &str) -> bool {
    matches!(database_url, "sqlite::memory:" | "sqlite://:memory:" | ":memory:")
        || database_url.contains("mode=memory")
}

pub async fn migrate(pool: &SqlitePool) -> anyhow::Result<()> {
    MIGRATOR.run(pool).await?;
    Ok(())
}

pub async fn migrate_content(pool: &SqlitePool) -> anyhow::Result<()> {
    CONTENT_MIGRATOR.run(pool).await?;
    Ok(())
}

/// 读取连接实际生效的 journal 模式。
///
/// `PRAGMA journal_mode=WAL` 在无法转换时**不报错**：SQLite 把实际生效的模式作为结果行返回，
/// 而 `execute` 会把这一行丢掉。不支持共享内存的文件系统（网络共享、部分同步盘）上，
/// 程序会静默留在回滚日志模式。桌面端数据库就放在可执行文件同目录，便携安装被放进同步盘
/// 并不罕见，因此并发写之前必须实测一次而不是假定成功。
pub async fn effective_journal_mode(pool: &SqlitePool) -> anyhow::Result<String> {
    let mode: String = sqlx::query_scalar("PRAGMA journal_mode").fetch_one(pool).await?;
    Ok(mode.to_ascii_lowercase())
}

/// 文件库的连接池大小。
///
/// 必须**大于**刷新并发度：每个刷新任务在批量写入期间会独占一条连接（整批包在一个事务里），
/// 如果池大小恰好等于并发度，刷新任务就能把池占满，界面查询和正文图片本地化的写回只能排队
/// 等连接——表现为刷新时界面卡住。这里直接由
/// [`rssr_application::DEFAULT_REFRESH_CONCURRENCY`] 推导，避免两个数字各自漂移。
///
/// 余量留给：UI 的列表/计数查询、阅读页正文读取、以及图片本地化的写回。
///
/// 内存库只能用 1：`sqlite::memory:` 的每条连接都是各自独立的空库。
pub(crate) fn default_sqlite_max_connections(database_url: &str) -> u32 {
    const POOL_HEADROOM: usize = 4;

    if is_memory_database(database_url) {
        1
    } else {
        (rssr_application::DEFAULT_REFRESH_CONCURRENCY + POOL_HEADROOM) as u32
    }
}
