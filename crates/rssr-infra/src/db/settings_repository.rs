use rssr_domain::{DomainError, Result as DomainResult, SettingsRepository, UserSettings};
use sqlx::Row;
use time::OffsetDateTime;

use crate::db::SqlitePool;

#[derive(Clone)]
pub struct SqliteSettingsRepository {
    pool: SqlitePool,
}

impl SqliteSettingsRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl SettingsRepository for SqliteSettingsRepository {
    async fn load(&self) -> DomainResult<UserSettings> {
        let row = sqlx::query("SELECT value FROM app_settings WHERE key = 'user_settings'")
            .fetch_optional(&self.pool)
            .await
            .map_err(map_sqlx_error)?;

        match row {
            Some(row) => {
                let raw: String = row.try_get("value").map_err(map_sqlx_error)?;
                serde_json::from_str(&raw)
                    .map_err(|error| DomainError::Persistence(error.to_string()))
            }
            None => Ok(UserSettings::default()),
        }
    }

    async fn save(&self, settings: &UserSettings) -> DomainResult<()> {
        let raw = serde_json::to_string(settings)
            .map_err(|error| DomainError::Persistence(error.to_string()))?;
        let now = OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .expect("format current time");

        sqlx::query(
            r#"
            INSERT INTO app_settings (key, value, updated_at)
            VALUES ('user_settings', ?1, ?2)
            ON CONFLICT(key) DO UPDATE SET
                value = excluded.value,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(raw)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(())
    }
}

fn map_sqlx_error(error: sqlx::Error) -> DomainError {
    DomainError::Persistence(error.to_string())
}
