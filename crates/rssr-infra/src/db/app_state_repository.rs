use rssr_domain::{DomainError, Result as DomainResult};
use sqlx::Row;
use time::OffsetDateTime;

use crate::db::SqlitePool;

const LAST_OPENED_FEED_KEY: &str = "last_opened_feed_id";

#[derive(Clone)]
pub struct SqliteAppStateRepository {
    pool: SqlitePool,
}

impl SqliteAppStateRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn load_last_opened_feed_id(&self) -> DomainResult<Option<i64>> {
        let row = sqlx::query("SELECT value FROM app_settings WHERE key = ?1")
            .bind(LAST_OPENED_FEED_KEY)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_sqlx_error)?;

        match row {
            Some(row) => {
                let raw: String = row.try_get("value").map_err(map_sqlx_error)?;
                raw.parse::<i64>()
                    .map(Some)
                    .map_err(|error| DomainError::Persistence(error.to_string()))
            }
            None => Ok(None),
        }
    }

    pub async fn save_last_opened_feed_id(&self, feed_id: Option<i64>) -> DomainResult<()> {
        match feed_id {
            Some(feed_id) => {
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
                .bind(LAST_OPENED_FEED_KEY)
                .bind(feed_id.to_string())
                .bind(now)
                .execute(&self.pool)
                .await
                .map_err(map_sqlx_error)?;
            }
            None => {
                sqlx::query("DELETE FROM app_settings WHERE key = ?1")
                    .bind(LAST_OPENED_FEED_KEY)
                    .execute(&self.pool)
                    .await
                    .map_err(map_sqlx_error)?;
            }
        }

        Ok(())
    }
}

fn map_sqlx_error(error: sqlx::Error) -> DomainError {
    DomainError::Persistence(error.to_string())
}
