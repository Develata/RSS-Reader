use async_trait::async_trait;

use crate::db::SqlitePool;

#[async_trait]
pub trait StorageBackend: Send + Sync {
    async fn connect(&self) -> anyhow::Result<SqlitePool>;
    async fn migrate(&self, pool: &SqlitePool) -> anyhow::Result<()>;
    fn label(&self) -> &'static str;
}
