use async_trait::async_trait;

use crate::db::{SqlitePool, create_sqlite_pool, migrate, storage_backend::StorageBackend};

#[derive(Debug, Clone)]
pub struct WebSqliteBackend {
    database_url: String,
}

impl Default for WebSqliteBackend {
    fn default() -> Self {
        Self::new("sqlite::memory:")
    }
}

impl WebSqliteBackend {
    pub fn new(database_url: impl Into<String>) -> Self {
        Self { database_url: database_url.into() }
    }
}

#[async_trait]
impl StorageBackend for WebSqliteBackend {
    async fn connect(&self) -> anyhow::Result<SqlitePool> {
        tracing::warn!(
            backend = self.label(),
            database_url = %self.database_url,
            "当前 Web 存储后端仍使用 SQLite 连接占位；后续会替换为 IndexedDB 持久化桥接"
        );
        create_sqlite_pool(&self.database_url).await
    }

    async fn migrate(&self, pool: &SqlitePool) -> anyhow::Result<()> {
        migrate(pool).await
    }

    fn label(&self) -> &'static str {
        "sqlite-web"
    }
}
