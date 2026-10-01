use anyhow::{Context, Result, bail};
use rssr_application::{
    ConfigReplacementOutcome, ConfigReplacementPlan, ConfigReplacementPort, SubscriptionRemovalPort,
};
use rssr_domain::{AppStateSnapshot, UserSettings};
use sqlx::Row;
use time::OffsetDateTime;

use crate::db::SqlitePool;

const APP_STATE_KEY: &str = "app_state_v2";

pub async fn cleanup_deleted_feed_content(
    index_pool: &SqlitePool,
    content_pool: &SqlitePool,
) -> Result<u64> {
    // Keep the index writer lock while deleting the separate content cache. Otherwise another
    // process could re-activate the same feed between the tombstone check and content deletion.
    // Refresh and removal already acquire locks in index -> content order, so this preserves the
    // existing lock ordering rather than introducing an inversion.
    let mut tx =
        index_pool.begin_with("BEGIN IMMEDIATE").await.context("开始已删除正文清理事务失败")?;
    let feed_ids = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT feeds.id
        FROM feeds
        WHERE feeds.is_deleted = 1
          AND NOT EXISTS (
              SELECT 1 FROM entries WHERE entries.feed_id = feeds.id
          )
        "#,
    )
    .fetch_all(&mut *tx)
    .await
    .context("读取待清理正文的已删除订阅失败")?;

    let mut removed_rows = 0_u64;
    for feed_id in feed_ids {
        removed_rows += sqlx::query("DELETE FROM entry_contents WHERE feed_id = ?1")
            .bind(feed_id)
            .execute(content_pool)
            .await
            .with_context(|| format!("重试清理订阅 {feed_id} 的正文缓存失败"))?
            .rows_affected();
    }
    tx.commit().await.context("完成已删除正文清理事务失败")?;
    Ok(removed_rows)
}

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
        let mut tx =
            self.index_pool.begin_with("BEGIN IMMEDIATE").await.context("开始订阅删除事务失败")?;
        let now = now_rfc3339();

        let result = sqlx::query("UPDATE feeds SET is_deleted = 1, updated_at = ?2 WHERE id = ?1")
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
    async fn replace_config(
        &self,
        plan: ConfigReplacementPlan,
    ) -> Result<ConfigReplacementOutcome> {
        let settings_raw = serde_json::to_string(&plan.settings).context("序列化导入设置失败")?;
        let now = now_rfc3339();

        let mut tx =
            self.index_pool.begin_with("BEGIN IMMEDIATE").await.context("开始配置替换事务失败")?;

        let current_rows = sqlx::query("SELECT id, url FROM feeds WHERE is_deleted = 0")
            .fetch_all(&mut *tx)
            .await
            .context("读取当前订阅失败")?;
        let desired_urls = plan
            .feeds
            .iter()
            .map(|feed| feed.url.as_str())
            .collect::<std::collections::HashSet<_>>();
        let removed_feed_ids = current_rows
            .iter()
            .filter_map(|row| {
                let url: String = row.get("url");
                (!desired_urls.contains(url.as_str())).then(|| row.get::<i64, _>("id"))
            })
            .collect::<Vec<_>>();

        let current_settings =
            match sqlx::query("SELECT value FROM app_settings WHERE key = 'user_settings'")
                .fetch_optional(&mut *tx)
                .await
                .context("读取当前设置失败")?
            {
                Some(row) => {
                    let raw: String = row.try_get("value").context("读取当前设置内容失败")?;
                    serde_json::from_str::<UserSettings>(&raw).context("解析当前设置失败")?
                }
                None => UserSettings::default(),
            };
        let settings_updated = current_settings != plan.settings;

        for feed in &plan.feeds {
            upsert_config_feed(&mut tx, feed, &now).await?;
        }

        for &feed_id in &removed_feed_ids {
            let result =
                sqlx::query("UPDATE feeds SET is_deleted = 1, updated_at = ?2 WHERE id = ?1")
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

        clear_last_opened_if_matches(&mut tx, &removed_feed_ids, &now).await?;
        tx.commit().await.context("提交配置替换事务失败")?;

        self.cleanup_content_best_effort(&removed_feed_ids).await;
        Ok(ConfigReplacementOutcome {
            removed_feed_count: removed_feed_ids.len(),
            settings_updated,
        })
    }
}

async fn upsert_config_feed(
    connection: &mut sqlx::SqliteConnection,
    feed: &rssr_application::ConfigReplacementFeed,
    now: &str,
) -> Result<()> {
    let title = feed.title.as_deref().map(str::trim).filter(|value| !value.is_empty());
    let folder = feed.folder.as_deref().map(str::trim).filter(|value| !value.is_empty());

    sqlx::query(
        r#"
        INSERT INTO feeds (url, title, folder, created_at, updated_at, site_url)
        VALUES (?1, ?2, ?3, ?4, ?4, NULL)
        ON CONFLICT(url) DO UPDATE SET
            title = excluded.title,
            folder = excluded.folder,
            is_deleted = 0,
            updated_at = excluded.updated_at
        "#,
    )
    .bind(feed.url.as_str())
    .bind(title)
    .bind(folder)
    .bind(now)
    .execute(connection)
    .await
    .with_context(|| format!("保存订阅 {} 失败", feed.url))?;
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
    let mut state: AppStateSnapshot = serde_json::from_str(&raw).context("解析应用状态失败")?;
    if state.last_opened_feed_id.is_none_or(|feed_id| !removed_feed_ids.contains(&feed_id)) {
        return Ok(());
    }

    state.last_opened_feed_id = None;
    let raw = serde_json::to_string(&state).context("序列化应用状态失败")?;
    sqlx::query("UPDATE app_settings SET value = ?2, updated_at = ?3 WHERE key = ?1")
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
