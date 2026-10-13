//! Refresh persistence: targets for the refresh service and one write transaction per commit.
//!
//! A commit is all or nothing: the generation check, the feed metadata, every entry, every body
//! and the fetch state commit together, so a failed write never leaves new entries next to old
//! validators (in v0.1.x these were four transactions over two databases). Network and parsing
//! happen before the commit, outside any transaction.

use anyhow::{Context, Result};
use rssr_application::{
    FeedRefreshUpdate, ParsedEntryData, RefreshCommit, RefreshCommitOutcome, RefreshFailure,
    RefreshHttpMetadata, RefreshStorePort, RefreshTarget,
};
use rssr_domain::DomainError;
use rusqlite::{Connection, params};
use url::Url;

use super::codec::{format_optional_time, now};
use super::error::{StoreError, StoreResult};
use super::executor::LibraryStore;
use super::feeds::{self, FeedRow, active_generation};
use crate::feed_normalization::hash_content;

/// The refresh target for a subscription. Conditional-request validators are sent only when the
/// last refresh succeeded and entries are cached: a failure's validators describe a response that
/// was never stored, and resending them could answer 304 forever.
fn target_from(row: FeedRow, has_entries: bool) -> RefreshTarget {
    let FeedRow { feed, generation } = row;
    let (etag, last_modified) = if has_entries && feed.fetch_error.is_none() {
        (feed.etag, feed.last_modified)
    } else {
        tracing::debug!(
            feed_id = feed.id,
            url = %feed.url,
            "订阅本地无文章缓存或上次刷新失败，跳过条件请求并强制全量抓取"
        );
        (None, None)
    };
    RefreshTarget {
        feed_id: feed.id,
        generation,
        url: feed.url,
        etag,
        last_modified,
        ticket: Default::default(),
    }
}

fn has_entries(conn: &Connection, feed_id: i64) -> StoreResult<bool> {
    Ok(conn
        .prepare_cached("SELECT EXISTS(SELECT 1 FROM entries WHERE feed_id = ?1)")?
        .query_row([feed_id], |row| row.get(0))?)
}

fn list_targets(conn: &Connection) -> StoreResult<Vec<RefreshTarget>> {
    let feeds = feeds::list_active(conn)?;
    let mut targets = Vec::with_capacity(feeds.len());
    for row in feeds {
        let has_entries = has_entries(conn, row.feed.id)?;
        targets.push(target_from(row, has_entries));
    }
    Ok(targets)
}

fn get_target(conn: &Connection, feed_id: i64) -> StoreResult<Option<RefreshTarget>> {
    let Some(row) = feeds::get_active(conn, feed_id)? else { return Ok(None) };
    let has_entries = has_entries(conn, feed_id)?;
    Ok(Some(target_from(row, has_entries)))
}

/// Fails with `NotFound` unless the subscription is active in the target's generation: a
/// refresh never writes into a subscription that was removed, or removed and added again.
fn ensure_generation(conn: &Connection, target: &RefreshTarget) -> StoreResult<()> {
    match active_generation(conn, target.feed_id)? {
        Some(generation) if generation == target.generation => Ok(()),
        _ => Err(StoreError::NotFound),
    }
}

fn record_fetch_state(
    conn: &Connection,
    target: &RefreshTarget,
    metadata: Option<&RefreshHttpMetadata>,
    fetch_error: Option<&str>,
    now: &str,
) -> StoreResult<()> {
    let changed = conn
        .prepare_cached(
            "UPDATE feeds
             SET etag = ?3, last_modified = ?4, last_fetched_at = ?5,
                 last_success_at = CASE WHEN ?6 IS NULL THEN ?5 ELSE last_success_at END,
                 fetch_error = ?6, updated_at = ?5
             WHERE id = ?1 AND generation = ?2 AND is_deleted = 0",
        )?
        .execute(params![
            target.feed_id,
            target.generation,
            metadata.and_then(|metadata| metadata.etag.as_deref()),
            metadata.and_then(|metadata| metadata.last_modified.as_deref()),
            now,
            fetch_error,
        ])?;
    if changed == 0 {
        return Err(StoreError::NotFound);
    }
    Ok(())
}

/// A 304 confirms the cache: RFC 9110 lets the server omit validators it does not resend, and
/// the stored ones must survive or the next request loses its conditions.
fn record_not_modified(
    conn: &Connection,
    target: &RefreshTarget,
    metadata: &RefreshHttpMetadata,
    now: &str,
) -> StoreResult<()> {
    let changed = conn
        .prepare_cached(
            "UPDATE feeds
             SET etag = COALESCE(?3, etag), last_modified = COALESCE(?4, last_modified),
                 last_fetched_at = ?5, last_success_at = ?5, fetch_error = NULL, updated_at = ?5
             WHERE id = ?1 AND generation = ?2 AND is_deleted = 0",
        )?
        .execute(params![
            target.feed_id,
            target.generation,
            metadata.etag.as_deref(),
            metadata.last_modified.as_deref(),
            now,
        ])?;
    if changed == 0 {
        return Err(StoreError::NotFound);
    }
    Ok(())
}

/// Versions before the GUID provenance fix stored legitimate 32/40/64-hex source GUIDs with both
/// identity columns replaced by the entry URL. Promote exactly that legacy row to the GUID
/// identity; never guess across older link changes.
fn promote_legacy_hex_guid_identity(
    conn: &Connection,
    feed_id: i64,
    entry: &ParsedEntryData,
) -> StoreResult<()> {
    let Some(url) = entry.url.as_ref().map(Url::as_str) else { return Ok(()) };
    if entry.external_id != entry.dedup_key
        || entry.dedup_key == url
        || !matches!(entry.dedup_key.len(), 32 | 40 | 64)
        || !entry.dedup_key.chars().all(|ch| ch.is_ascii_hexdigit())
    {
        return Ok(());
    }
    let current_identity_exists: bool = conn
        .prepare_cached(
            "SELECT EXISTS(SELECT 1 FROM entries
                           WHERE feed_id = ?1 AND (external_id = ?2 OR dedup_key = ?2))",
        )?
        .query_row(params![feed_id, entry.dedup_key], |row| row.get(0))?;
    if current_identity_exists {
        return Ok(());
    }
    conn.prepare_cached(
        "UPDATE entries SET external_id = ?2, dedup_key = ?2
         WHERE id = (SELECT id FROM entries
                     WHERE feed_id = ?1 AND external_id = ?3 AND dedup_key = ?3 AND url = ?3
                     LIMIT 1)",
    )?
    .execute(params![feed_id, entry.dedup_key, url])?;
    Ok(())
}

/// Writes one refreshed entry and its body.
///
/// The body's `content_hash` identifies the source body. When the source is unchanged the stored
/// body stays (it may be the localized derivative); a changed source replaces it (audit C1).
fn upsert_entry(
    conn: &Connection,
    feed_id: i64,
    entry: &ParsedEntryData,
    now: &str,
) -> StoreResult<()> {
    promote_legacy_hex_guid_identity(conn, feed_id, entry)?;
    let has_body = entry.content_html.is_some() || entry.content_text.is_some();
    let entry_id: i64 = conn
        .prepare_cached(
            "INSERT INTO entries (
                 feed_id, external_id, dedup_key, url, title, title_search, author, summary,
                 published_at, updated_at_source, first_seen_at, has_content, created_at,
                 updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?11, ?11)
             ON CONFLICT(feed_id, dedup_key) DO UPDATE SET
                 external_id = excluded.external_id,
                 url = COALESCE(excluded.url, entries.url),
                 title = excluded.title,
                 title_search = excluded.title_search,
                 author = excluded.author,
                 summary = excluded.summary,
                 published_at = COALESCE(excluded.published_at, entries.published_at),
                 updated_at_source = COALESCE(excluded.updated_at_source, entries.updated_at_source),
                 has_content = MAX(entries.has_content, excluded.has_content),
                 updated_at = excluded.updated_at
             RETURNING id",
        )?
        .query_row(
            params![
                feed_id,
                entry.external_id,
                entry.dedup_key,
                entry.url.as_ref().map(Url::as_str),
                entry.title,
                rssr_domain::fold_for_search(&entry.title),
                entry.author,
                entry.summary,
                format_optional_time(entry.published_at)?,
                format_optional_time(entry.updated_at_source)?,
                now,
                has_body,
            ],
            |row| row.get(0),
        )?;
    if has_body {
        conn.prepare_cached(
            "INSERT INTO entry_contents (entry_id, content_html, content_text, content_hash,
                                         updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(entry_id) DO UPDATE SET
                 content_html = COALESCE(excluded.content_html, entry_contents.content_html),
                 content_text = COALESCE(excluded.content_text, entry_contents.content_text),
                 content_hash = excluded.content_hash,
                 updated_at = excluded.updated_at
             WHERE entry_contents.content_hash IS NOT excluded.content_hash",
        )?
        .execute(params![
            entry_id,
            entry.content_html,
            entry.content_text,
            hash_content(
                entry.content_html.as_deref(),
                entry.content_text.as_deref(),
                Some(&entry.title),
                entry.url.as_ref().map(Url::as_str),
            ),
            now,
        ])?;
    }
    Ok(())
}

fn entry_count(conn: &Connection, feed_id: i64) -> StoreResult<u64> {
    let count: i64 = conn
        .prepare_cached("SELECT COUNT(*) FROM entries WHERE feed_id = ?1")?
        .query_row([feed_id], |row| row.get(0))?;
    Ok(count as u64)
}

/// Applies a fetched feed: metadata, entries, bodies and validators. Returns the number of new
/// entries.
fn commit_update(
    conn: &Connection,
    target: &RefreshTarget,
    update: &FeedRefreshUpdate,
    now: &str,
) -> StoreResult<u64> {
    ensure_generation(conn, target)?;
    let feed = &update.feed;
    conn.prepare_cached(
        "UPDATE feeds
         SET title = COALESCE(?2, title), site_url = COALESCE(?3, site_url),
             description = COALESCE(?4, description), updated_at = ?5
         WHERE id = ?1",
    )?
    .execute(params![
        target.feed_id,
        feed.title,
        feed.site_url.as_ref().map(Url::as_str),
        feed.description,
        now,
    ])?;
    // The difference in rows counts only real insertions: updates and keys repeated within the
    // batch do not count.
    let before = entry_count(conn, target.feed_id)?;
    for entry in &feed.entries {
        upsert_entry(conn, target.feed_id, entry, now)?;
    }
    let inserted_count = entry_count(conn, target.feed_id)? - before;
    record_fetch_state(conn, target, Some(&update.metadata), None, now)?;
    Ok(inserted_count)
}

/// [`RefreshStorePort`] over the library.
#[derive(Clone)]
pub struct LibraryRefreshStore {
    store: LibraryStore,
}

impl LibraryRefreshStore {
    pub fn new(store: LibraryStore) -> Self {
        Self { store }
    }

    async fn record_failure(&self, target: &RefreshTarget, failure: RefreshFailure) -> Result<()> {
        let target = target.clone();
        self.store
            .write(move |tx| {
                record_fetch_state(
                    tx,
                    &target,
                    failure.metadata.as_ref(),
                    Some(&failure.message),
                    &now(),
                )
            })
            .await
            .map_err(DomainError::from)
            .context("更新订阅抓取状态失败")
    }
}

#[async_trait::async_trait]
impl RefreshStorePort for LibraryRefreshStore {
    async fn list_targets(&self) -> Result<Vec<RefreshTarget>> {
        Ok(self.store.read(list_targets).await.map_err(DomainError::from)?)
    }

    async fn get_target(&self, feed_id: i64) -> Result<Option<RefreshTarget>> {
        Ok(self
            .store
            .read(move |conn| get_target(conn, feed_id))
            .await
            .map_err(DomainError::from)?)
    }

    async fn commit(
        &self,
        target: &RefreshTarget,
        commit: RefreshCommit,
    ) -> Result<RefreshCommitOutcome> {
        match commit {
            RefreshCommit::NotModified { metadata } => {
                let target = target.clone();
                self.store
                    .write(move |tx| record_not_modified(tx, &target, &metadata, &now()))
                    .await
                    .map_err(DomainError::from)
                    .context("更新订阅抓取状态失败")?;
                Ok(RefreshCommitOutcome::default())
            }
            RefreshCommit::Updated { update } => {
                let metadata = update.metadata.clone();
                let owned_target = target.clone();
                let written = self
                    .store
                    .write(move |tx| commit_update(tx, &owned_target, &update, &now()))
                    .await;
                match written {
                    Ok(inserted_count) => {
                        Ok(RefreshCommitOutcome { inserted_count, superseded: false })
                    }
                    Err(error) => {
                        let failure = RefreshFailure {
                            message: format!("写入订阅内容失败: {error}"),
                            metadata: Some(metadata),
                        };
                        let _ = self.record_failure(target, failure).await;
                        Err(anyhow::Error::new(DomainError::from(error))
                            .context("写入订阅内容失败"))
                    }
                }
            }
            RefreshCommit::Failed { failure } => {
                self.record_failure(target, failure).await?;
                Ok(RefreshCommitOutcome::default())
            }
        }
    }
}
