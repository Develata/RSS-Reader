use anyhow::{Context, Result, bail};
use rssr_application::{
    ActivatedSubscription, ConfigReplacementOutcome, ConfigReplacementPlan, ConfigReplacementPort,
    SubscriptionActivationPort, SubscriptionRemovalPort,
};
use rssr_domain::{AppStateSnapshot, NewFeedSubscription, UserSettings};
use sqlx::{QueryBuilder, Row, Sqlite};
use time::OffsetDateTime;

use crate::db::{SqlitePool, feed_repository::SqliteFeedRepository};

const APP_STATE_KEY: &str = "app_state_v2";

const CONTENT_GC_BATCH: usize = 900;

async fn live_entry_ids(
    index_pool: &SqlitePool,
    entry_ids: &[i64],
) -> Result<std::collections::HashSet<i64>> {
    if entry_ids.is_empty() {
        return Ok(std::collections::HashSet::new());
    }
    let mut query = QueryBuilder::<Sqlite>::new("SELECT id FROM entries WHERE id IN (");
    let mut separated = query.separated(", ");
    for entry_id in entry_ids {
        separated.push_bind(*entry_id);
    }
    query.push(")");
    Ok(query
        .build()
        .fetch_all(index_pool)
        .await
        .context("核对正文缓存索引归属失败")?
        .into_iter()
        .map(|row| row.get::<i64, _>("id"))
        .collect())
}

async fn delete_content_entry_ids(content_pool: &SqlitePool, entry_ids: &[i64]) -> Result<u64> {
    if entry_ids.is_empty() {
        return Ok(0);
    }
    let mut tx = content_pool.begin().await.context("开始正文缓存清理事务失败")?;
    let mut removed_rows = 0_u64;
    for chunk in entry_ids.chunks(CONTENT_GC_BATCH) {
        let mut delete =
            QueryBuilder::<Sqlite>::new("DELETE FROM entry_contents WHERE entry_id IN (");
        let mut separated = delete.separated(", ");
        for entry_id in chunk {
            separated.push_bind(*entry_id);
        }
        delete.push(")");
        removed_rows += delete
            .build()
            .execute(&mut *tx)
            .await
            .context("删除待清理正文缓存失败")?
            .rows_affected();
    }
    tx.commit().await.context("提交正文缓存清理事务失败")?;
    Ok(removed_rows)
}

async fn reconcile_legacy_orphans_once(
    index_pool: &SqlitePool,
    content_pool: &SqlitePool,
) -> Result<u64> {
    let reconciled: i64 =
        sqlx::query_scalar("SELECT legacy_reconciled FROM content_gc_state WHERE id = 1")
            .fetch_one(index_pool)
            .await
            .context("读取正文缓存迁移状态失败")?;
    if reconciled != 0 {
        return Ok(0);
    }

    // Older builds could lose the retry target when a purge failed and the feed was reactivated.
    // Do one bounded full reconciliation after the migration, then rely on the durable queue.
    let high_watermark =
        sqlx::query_scalar::<_, Option<i64>>("SELECT MAX(entry_id) FROM entry_contents")
            .fetch_one(content_pool)
            .await
            .context("读取正文缓存清理高水位失败")?;
    let mut removed_rows = 0_u64;

    if let Some(high_watermark) = high_watermark {
        let mut last_entry_id = i64::MIN;
        while last_entry_id < high_watermark {
            let entry_ids = sqlx::query_scalar::<_, i64>(
                r#"
                SELECT entry_id
                FROM entry_contents
                WHERE entry_id > ?1 AND entry_id <= ?2
                ORDER BY entry_id
                LIMIT ?3
                "#,
            )
            .bind(last_entry_id)
            .bind(high_watermark)
            .bind(CONTENT_GC_BATCH as i64)
            .fetch_all(content_pool)
            .await
            .context("读取旧版正文缓存候选项失败")?;
            let Some(&batch_last_entry_id) = entry_ids.last() else {
                break;
            };
            last_entry_id = batch_last_entry_id;

            let live = live_entry_ids(index_pool, &entry_ids).await?;
            let orphan_ids = entry_ids
                .into_iter()
                .filter(|entry_id| !live.contains(entry_id))
                .collect::<Vec<_>>();
            removed_rows += delete_content_entry_ids(content_pool, &orphan_ids).await?;
        }
    }

    sqlx::query("UPDATE content_gc_state SET legacy_reconciled = 1 WHERE id = 1")
        .execute(index_pool)
        .await
        .context("记录正文缓存迁移完成状态失败")?;
    Ok(removed_rows)
}

pub async fn cleanup_pending_entry_content(
    index_pool: &SqlitePool,
    content_pool: &SqlitePool,
) -> Result<u64> {
    let mut removed_rows = reconcile_legacy_orphans_once(index_pool, content_pool).await?;

    loop {
        let entry_ids = sqlx::query_scalar::<_, i64>(
            "SELECT entry_id FROM content_gc_queue ORDER BY entry_id LIMIT ?1",
        )
        .bind(CONTENT_GC_BATCH as i64)
        .fetch_all(index_pool)
        .await
        .context("读取待清理正文队列失败")?;
        if entry_ids.is_empty() {
            break;
        }

        // Delete content first. If clearing the queue fails or the process crashes afterwards,
        // retrying is idempotent and the durable queue still preserves the cleanup obligation.
        removed_rows += delete_content_entry_ids(content_pool, &entry_ids).await?;

        let mut delete_queue =
            QueryBuilder::<Sqlite>::new("DELETE FROM content_gc_queue WHERE entry_id IN (");
        let mut separated = delete_queue.separated(", ");
        for entry_id in &entry_ids {
            separated.push_bind(*entry_id);
        }
        delete_queue.push(")");
        delete_queue.build().execute(index_pool).await.context("确认正文缓存清理队列失败")?;
    }

    Ok(removed_rows)
}

async fn queue_feed_entry_content_cleanup(
    connection: &mut sqlx::SqliteConnection,
    feed_id: i64,
    queued_at: &str,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT OR IGNORE INTO content_gc_queue (entry_id, queued_at)
        SELECT id, ?2 FROM entries WHERE feed_id = ?1
        "#,
    )
    .bind(feed_id)
    .bind(queued_at)
    .execute(connection)
    .await
    .with_context(|| format!("登记订阅 {feed_id} 的正文缓存清理任务失败"))?;
    Ok(())
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

    async fn retry_pending_content_cleanup(&self) {
        if let Err(error) =
            cleanup_pending_entry_content(&self.index_pool, &self.content_pool).await
        {
            tracing::warn!(
                error = %error,
                "订阅已从索引库原子删除，但正文缓存清理失败；将在下次安全重试"
            );
        }
    }
}

#[async_trait::async_trait]
impl SubscriptionActivationPort for SqlitePersistenceMutations {
    async fn activate_subscription(
        &self,
        new_feed: NewFeedSubscription,
    ) -> Result<ActivatedSubscription> {
        let repository = SqliteFeedRepository::new(self.index_pool.clone());
        let (feed, generation) =
            repository.activate_subscription_with_generation(&new_feed, true).await?;
        Ok(ActivatedSubscription { feed, generation })
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
            queue_feed_entry_content_cleanup(&mut tx, feed_id, &now).await?;
            sqlx::query("DELETE FROM entries WHERE feed_id = ?1")
                .bind(feed_id)
                .execute(&mut *tx)
                .await
                .context("删除文章索引失败")?;
        }
        clear_last_opened_if_matches(&mut tx, &[feed_id], &now).await?;
        tx.commit().await.context("提交订阅删除事务失败")?;

        if purge_entries {
            // The index transaction durably records immutable entry ids before deleting them.
            // Cleanup therefore survives failure and same-URL reactivation without holding the
            // index writer lock while the separate content database is busy.
            self.retry_pending_content_cleanup().await;
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
            queue_feed_entry_content_cleanup(&mut tx, feed_id, &now).await?;
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

        if !removed_feed_ids.is_empty() {
            self.retry_pending_content_cleanup().await;
        }
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
            site_url = CASE WHEN feeds.is_deleted = 1 THEN NULL ELSE feeds.site_url END,
            description = CASE WHEN feeds.is_deleted = 1 THEN NULL ELSE feeds.description END,
            icon_url = CASE WHEN feeds.is_deleted = 1 THEN NULL ELSE feeds.icon_url END,
            etag = CASE WHEN feeds.is_deleted = 1 THEN NULL ELSE feeds.etag END,
            last_modified = CASE WHEN feeds.is_deleted = 1 THEN NULL ELSE feeds.last_modified END,
            last_fetched_at = CASE WHEN feeds.is_deleted = 1 THEN NULL ELSE feeds.last_fetched_at END,
            last_success_at = CASE WHEN feeds.is_deleted = 1 THEN NULL ELSE feeds.last_success_at END,
            fetch_error = CASE WHEN feeds.is_deleted = 1 THEN NULL ELSE feeds.fetch_error END,
            generation = CASE
                WHEN feeds.is_deleted = 1 THEN feeds.generation + 1
                ELSE feeds.generation
            END,
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
