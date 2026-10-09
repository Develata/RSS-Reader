use rssr_domain::{
    AppStateMutation, AppStateRepository, AppStateSnapshot, DomainError, Result as DomainResult,
};
use sqlx::{Row, SqliteConnection};
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
        let mut connection = self.pool.acquire().await.map_err(map_sqlx_error)?;
        read_snapshot(&mut connection).await
    }

    /// 在同一个 IMMEDIATE 事务里读出、修改并写回整份状态。
    ///
    /// 写锁从读取前一直持有到提交，并发的窄字段更新（文章页偏好、最后打开的订阅）因此逐个
    /// 进行，谁也不会用自己读到的旧快照覆盖对方（审计 A2）。状态没有变化时不写。
    pub async fn update_snapshot(
        &self,
        mutate: impl FnOnce(&mut AppStateSnapshot),
    ) -> DomainResult<()> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await.map_err(map_sqlx_error)?;
        let before = read_snapshot(&mut tx).await?;
        let mut state = before.clone();
        mutate(&mut state);
        if state != before {
            write_snapshot(&mut tx, &state).await?;
        }
        tx.commit().await.map_err(map_sqlx_error)
    }

    pub async fn load_last_opened_feed_id(&self) -> DomainResult<Option<i64>> {
        Ok(self.load_snapshot().await?.last_opened_feed_id)
    }

    pub async fn save_last_opened_feed_id(&self, feed_id: Option<i64>) -> DomainResult<()> {
        self.update_snapshot(|state| state.last_opened_feed_id = feed_id).await
    }
}

#[async_trait::async_trait]
impl AppStateRepository for SqliteAppStateRepository {
    async fn load(&self) -> DomainResult<AppStateSnapshot> {
        self.load_snapshot().await
    }

    async fn update(&self, mutate: AppStateMutation) -> DomainResult<()> {
        self.update_snapshot(mutate).await
    }
}

async fn read_snapshot(connection: &mut SqliteConnection) -> DomainResult<AppStateSnapshot> {
    let row = sqlx::query("SELECT value FROM app_settings WHERE key = ?1")
        .bind(APP_STATE_KEY)
        .fetch_optional(connection)
        .await
        .map_err(map_sqlx_error)?;

    match row {
        Some(row) => {
            let raw: String = row.try_get("value").map_err(map_sqlx_error)?;
            serde_json::from_str(&raw).map_err(|error| DomainError::Persistence(error.to_string()))
        }
        None => Ok(AppStateSnapshot::default()),
    }
}

async fn write_snapshot(
    connection: &mut SqliteConnection,
    state: &AppStateSnapshot,
) -> DomainResult<()> {
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
    .execute(connection)
    .await
    .map_err(map_sqlx_error)?;
    Ok(())
}

fn map_sqlx_error(error: sqlx::Error) -> DomainError {
    DomainError::Persistence(error.to_string())
}
