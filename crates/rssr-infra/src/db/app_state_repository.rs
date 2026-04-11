use rssr_domain::{AppStateRepository, AppStateSnapshot, DomainError, Result as DomainResult};
use sqlx::Row;
use time::OffsetDateTime;

use crate::db::SqlitePool;

const APP_STATE_KEY: &str = "app_state_v2";

#[derive(Clone)]
pub struct SqliteAppStateRepository {
    pool: SqlitePool,
}

impl SqliteAppStateRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn load_snapshot(&self) -> DomainResult<AppStateSnapshot> {
        let row = sqlx::query("SELECT value FROM app_settings WHERE key = ?1")
            .bind(APP_STATE_KEY)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_sqlx_error)?;

        match row {
            Some(row) => {
                let raw: String = row.try_get("value").map_err(map_sqlx_error)?;
                serde_json::from_str(&raw)
                    .map_err(|error| DomainError::Persistence(error.to_string()))
            }
            None => Ok(AppStateSnapshot::default()),
        }
    }

    pub async fn save_snapshot(&self, state: &AppStateSnapshot) -> DomainResult<()> {
        let raw = serde_json::to_string(state)
            .map_err(|error| DomainError::Persistence(error.to_string()))?;
        let now = OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .expect("format current time");

        sqlx::query(
            r#"
            INSERT INTO app_settings (key, value, updated_at)
            VALUES (?1, ?2, ?3)
            ON CONFLICT(key) DO UPDATE SET
                value = excluded.value,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(APP_STATE_KEY)
        .bind(raw)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(())
    }

    pub async fn load_last_opened_feed_id(&self) -> DomainResult<Option<i64>> {
        Ok(self.load_snapshot().await?.last_opened_feed_id)
    }

    pub async fn save_last_opened_feed_id(&self, feed_id: Option<i64>) -> DomainResult<()> {
        let mut state = self.load_snapshot().await?;
        state.last_opened_feed_id = feed_id;
        self.save_snapshot(&state).await
    }
}

#[async_trait::async_trait]
impl AppStateRepository for SqliteAppStateRepository {
    async fn load(&self) -> DomainResult<AppStateSnapshot> {
        self.load_snapshot().await
    }

    async fn save(&self, state: &AppStateSnapshot) -> DomainResult<()> {
        self.save_snapshot(state).await
    }
}

fn map_sqlx_error(error: sqlx::Error) -> DomainError {
    DomainError::Persistence(error.to_string())
}
