pub mod app_state_repository;
pub mod entry_repository;
pub mod feed_repository;
pub mod settings_repository;
pub mod sqlite_native;
pub mod storage_backend;

use sqlx::{Pool, Sqlite, sqlite::SqlitePoolOptions};

pub type SqlitePool = Pool<Sqlite>;

pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations");

pub async fn create_sqlite_pool(database_url: &str) -> anyhow::Result<SqlitePool> {
    let pool = SqlitePoolOptions::new()
        .max_connections(default_sqlite_max_connections(database_url))
        .connect(database_url)
        .await?;

    Ok(pool)
}

pub async fn migrate(pool: &SqlitePool) -> anyhow::Result<()> {
    MIGRATOR.run(pool).await?;
    Ok(())
}

pub(crate) fn default_sqlite_max_connections(database_url: &str) -> u32 {
    if database_url == "sqlite::memory:" || database_url.contains("mode=memory") { 1 } else { 4 }
}
