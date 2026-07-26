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

/// 写锁等待上限。刷新是并发的（见 `REFRESH_ALL_CONCURRENCY`），同一时刻只有一个写者能拿到
/// SQLite 的写锁，其余写者在这个时限内排队而不是立刻失败。
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

pub(crate) fn is_memory_database(database_url: &str) -> bool {
    database_url == "sqlite::memory:" || database_url.contains("mode=memory")
}

pub async fn migrate(pool: &SqlitePool) -> anyhow::Result<()> {
    MIGRATOR.run(pool).await?;
    Ok(())
}

pub async fn migrate_content(pool: &SqlitePool) -> anyhow::Result<()> {
    CONTENT_MIGRATOR.run(pool).await?;
    Ok(())
}

pub(crate) fn default_sqlite_max_connections(database_url: &str) -> u32 {
    if is_memory_database(database_url) { 1 } else { 4 }
}
