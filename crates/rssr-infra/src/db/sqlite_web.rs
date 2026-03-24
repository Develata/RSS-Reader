use async_trait::async_trait;

use crate::db::{storage_backend::StorageBackend, SqlitePool};

#[derive(Debug, Clone, Default)]
pub struct WebSqliteBackend;

#[async_trait]
impl StorageBackend for WebSqliteBackend {
    async fn connect(&self) -> anyhow::Result<SqlitePool> {
        Err(anyhow::anyhow!(
            "Web wasm SQLite + IndexedDB 后端将在 Web 目标实现阶段接入"
        ))
    }

    async fn migrate(&self, _pool: &SqlitePool) -> anyhow::Result<()> {
        Err(anyhow::anyhow!(
            "当前构建目标未启用 Web 持久化后端"
        ))
    }

    fn label(&self) -> &'static str {
        "sqlite-web"
    }
}
