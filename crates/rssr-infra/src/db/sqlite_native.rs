use async_trait::async_trait;

use crate::db::{create_sqlite_pool, migrate, storage_backend::StorageBackend, SqlitePool};

#[derive(Debug, Clone)]
pub struct NativeSqliteBackend {
    database_url: String,
}

impl NativeSqliteBackend {
    pub fn new(database_url: impl Into<String>) -> Self {
        Self {
            database_url: database_url.into(),
        }
    }
}

#[async_trait]
impl StorageBackend for NativeSqliteBackend {
    async fn connect(&self) -> anyhow::Result<SqlitePool> {
        create_sqlite_pool(&self.database_url).await
    }

    async fn migrate(&self, pool: &SqlitePool) -> anyhow::Result<()> {
        migrate(pool).await
    }

    fn label(&self) -> &'static str {
        "sqlite-native"
    }
}
