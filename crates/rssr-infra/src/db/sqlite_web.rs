use async_trait::async_trait;

use crate::db::{SqlitePool, create_sqlite_pool, migrate, storage_backend::StorageBackend};

#[derive(Debug, Clone)]
pub struct WebSqliteBackend {
    database_url: String,
}

impl Default for WebSqliteBackend {
    fn default() -> Self {
        Self::new(Self::default_database_url())
    }
}

impl WebSqliteBackend {
    pub const fn default_database_url() -> &'static str {
        "sqlite:rss-reader-web.db?mode=rwc"
    }

    pub fn new(database_url: impl Into<String>) -> Self {
        Self { database_url: database_url.into() }
    }

    pub fn database_url(&self) -> &str {
        &self.database_url
    }
}

#[async_trait]
impl StorageBackend for WebSqliteBackend {
    async fn connect(&self) -> anyhow::Result<SqlitePool> {
        tracing::warn!(
            backend = self.label(),
            database_url = %self.database_url,
            "当前 Web 存储后端默认使用持久化 SQLite 路径占位；后续会替换为 IndexedDB 持久化桥接"
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
