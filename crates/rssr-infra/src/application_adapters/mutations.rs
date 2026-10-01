use anyhow::{Context, Result, bail};
use rssr_application::{
    ConfigReplacementPlan, ConfigReplacementPort, SubscriptionRemovalPort,
};
use rssr_domain::{AppStateSnapshot, NewFeedSubscription};
use sqlx::Row;
use time::OffsetDateTime;

use crate::db::SqlitePool;

const APP_STATE_KEY: &str = "app_state_v2";

#[derive(Clone)]
pub struct SqlitePersistenceMutations {
    index_pool: SqlitePool,
    content_pool: SqlitePool,
}

impl SqlitePersistenceMutations {
    pub fn new(index_pool: SqlitePool, content_pool: SqlitePool) -> Self {
        Self { index_pool, content_pool }
    }

    async fn cleanup_content_best_effort(&self, feed_ids: &[i64]) {
        for &feed_id in feed_ids {
            if let Err(error) = sqlx::query("DELETE FROM entry_contents WHERE feed_id = ?1")
                .bind(feed_id)
                .execute(&self.content_pool)
                .await
            {
                tracing::warn!(
                    feed_id,
                    error = %error,
                    "订阅已从索引库原子删除，但正文缓存清理失败；后续可安全重试"
                );
            }
        }
    }
}

#[async_trait::async_trait]
impl SubscriptionRemovalPort for SqlitePersistenceMutations {
    async fn remove_subscription(&self, feed_id: i64, purge_entries: bool) -> Result<()> {
        let mut tx = self
            .index_pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .context("开始订阅删除事务失败")?;
        let now = now_rfc3339();

        let result = sqlx::query(
            "UPDATE feeds SET is_deleted = 1, updated_at = ?2 WHERE id = ?1",
        )
        .bind(feed_id)
        .bind(&now)
        .execute(&mut *tx)
        .await
        .context("标记订阅删除失败")?;
        if result.rows_affected() == 0 {
            bail!("订阅不存在");
        }

        if purge_entries {
            sqlx::query("DELETE FROM entries WHERE feed_id = ?1")
                .bind(feed_id)
                .execute(&mut *tx)
                .await
                .context("删除文章索引失败")?;
        }
        clear_last_opened_if_matches(&mut tx, &[feed_id], &now).await?;
        tx.commit().await.context("提交订阅删除事务失败")?;

        if purge_entries {
            self.cleanup_content_best_effort(&[feed_id]).await;
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl ConfigReplacementPort for SqlitePersistenceMutations {
    async fn replace_config(&self, plan: ConfigReplacementPlan) -> Result<()> {
        // Serialize before opening the write transaction so an impossible settings payload cannot
        // hold the SQLite writer lock or leave any prior feed mutation visible.
        let settings_raw =
            serde_json::to_string(&plan.settings).context("序列化导入设置失败")?;
        let now = now_rfc3339();

        let mut tx = self
            .index_pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .context("开始配置替换事务失败")?;

        for feed in &plan.upserts {
            upsert_subscription(&mut tx, feed, &now).await?;
        }

        for &feed_id in &plan.removed_feed_ids {
            let result = sqlx::query(
                "UPDATE feeds SET is_deleted = 1, updated_at = ?2 WHERE id = ?1",
            )
            .bind(feed_id)
            .bind(&now)
            .execute(&mut *tx)
            .await
            .with_context(|| format!("标记缺失订阅 {feed_id} 删除失败"))?;
            if result.rows_affected() == 0 {
                bail!("配置替换期间订阅 {feed_id} 不存在");
            }
            sqlx::query("DELETE FROM entries WHERE feed_id = ?1")
                .bind(feed_id)
                .execute(&mut *tx)
                .await
                .with_context(|| format!("删除订阅 {feed_id} 的文章索引失败"))?;
        }

        sqlx::query(
            r#"
            INSERT INTO app_settings (key, value, updated_at)
            VALUES ('user_settings', ?1, ?2)
            ON CONFLICT(key) DO UPDATE SET
                value = excluded.value,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(settings_raw)
        .bind(&now)
        .execute(&mut *tx)
        .await
        .context("保存导入设置失败")?;

        clear_last_opened_if_matches(&mut tx, &plan.removed_feed_ids, &now).await?;
        tx.commit().await.context("提交配置替换事务失败")?;

        self.cleanup_content_best_effort(&plan.removed_feed_ids).await;
        Ok(())
    }
}

async fn upsert_subscription(
    connection: &mut sqlx::SqliteConnection,
    new_feed: &NewFeedSubscription,
    now: &str,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO feeds (url, title, folder, created_at, updated_at, site_url)
        VALUES (?1, ?2, ?3, ?4, ?4, ?5)
        ON CONFLICT(url) DO UPDATE SET
            title = CASE
                WHEN excluded.title IS NULL THEN feeds.title
                ELSE NULLIF(excluded.title, '')
            END,
            folder = CASE
                WHEN excluded.folder IS NULL THEN feeds.folder
                ELSE NULLIF(excluded.folder, '')
            END,
            site_url = COALESCE(excluded.site_url, feeds.site_url),
            is_deleted = 0,
            updated_at = excluded.updated_at
        "#,
    )
    .bind(new_feed.url.as_str())
    .bind(new_feed.title.as_deref())
    .bind(new_feed.folder.as_deref())
    .bind(now)
    .bind(new_feed.site_url.as_ref().map(url::Url::as_str))
    .execute(connection)
    .await
    .with_context(|| format!("保存订阅 {} 失败", new_feed.url))?;
    Ok(())
}

async fn clear_last_opened_if_matches(
    connection: &mut sqlx::SqliteConnection,
    removed_feed_ids: &[i64],
    now: &str,
) -> Result<()> {
    if removed_feed_ids.is_empty() {
        return Ok(());
    }

    let row = sqlx::query("SELECT value FROM app_settings WHERE key = ?1")
        .bind(APP_STATE_KEY)
        .fetch_optional(&mut *connection)
        .await
        .context("读取应用状态失败")?;
    let Some(row) = row else {
        return Ok(());
    };

    let raw: String = row.try_get("value").context("读取应用状态内容失败")?;
    let mut state: AppStateSnapshot =
        serde_json::from_str(&raw).context("解析应用状态失败")?;
    if state
        .last_opened_feed_id
        .is_none_or(|feed_id| !removed_feed_ids.contains(&feed_id))
    {
        return Ok(());
    }

    state.last_opened_feed_id = None;
    let raw = serde_json::to_string(&state).context("序列化应用状态失败")?;
    sqlx::query(
        "UPDATE app_settings SET value = ?2, updated_at = ?3 WHERE key = ?1",
    )
    .bind(APP_STATE_KEY)
    .bind(raw)
    .bind(now)
    .execute(connection)
    .await
    .context("清理已删除订阅的最后打开状态失败")?;
    Ok(())
}

fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .expect("format current time")
}
