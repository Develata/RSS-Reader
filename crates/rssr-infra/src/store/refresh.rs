//! Refresh persistence: targets for the refresh service and one write transaction per commit.
//!
//! A commit is all or nothing: the generation check, the feed metadata, every entry, every body
//! and the fetch state commit together, so a failed write never leaves new entries next to old
//! validators (in v0.1.x these were four transactions over two databases). Network and parsing
//! happen before the commit, outside any transaction.
//!
//! Refreshes of one subscription are ordered by persistent tickets ([`RefreshTicket`]), so the
//! GUI, the CLI and the server sharing a library cannot overwrite a newer result with an older
//! one. Per subscription the library keeps the highest ticket issued (`refresh_issued`, I),
//! published (`refresh_published`, P) and diagnosed (`refresh_diagnosed`, D), `0 <= P <= D <= I`
//! (a CHECK constraint). A ticket is issued in the write transaction that starts a refresh,
//! together with the validators it sends and the P they belong to (its baseline). At commit:
//!
//! | response | applied when | effect |
//! |---|---|---|
//! | fetched feed | `t > P` | content and validators; `P := t`, `D := max(D, t)` |
//! | 304 | `baseline == P` | keeps the content; validators the 304 omits come from the request; `P := t`, `D := max(D, t)` |
//! | failure | `t > D` | error and fetch time; `D := t` |
//!
//! The fetch time and the error describe the newest diagnosis, so a publication with `t < D` (an
//! older success arriving after a newer failure) leaves them as they are. A commit that is not
//! applied writes nothing and reports itself superseded.
//!
//! Every commit carries the ticket of the request it answers: from `begin_refresh` (which
//! `get_target` also goes through) or from the activation of a new subscription. A commit without
//! one is refused: issuing a ticket only at commit time would put an earlier request after later
//! ones and give an old 304 a baseline it never had. Targets from `list_targets` only enumerate.
//!
//! A 304 completes the validators it omits from the ones the request sent (they belong to the
//! version P was when the ticket was issued), never from the stored row: a failure since then may
//! have stored the validators of a response that was never published.

use anyhow::{Context, Result};
use rssr_application::{
    FeedRefreshUpdate, ParsedEntryData, RefreshCommit, RefreshCommitOutcome, RefreshFailure,
    RefreshHttpMetadata, RefreshStorePort, RefreshTarget, RefreshTicket,
};
use rssr_domain::DomainError;
use rusqlite::{Connection, OptionalExtension, params};
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

/// Fails with `NotFound` unless the subscription is active in the target's generation: a
/// refresh never writes into a subscription that was removed, or removed and added again.
fn ensure_generation(conn: &Connection, target: &RefreshTarget) -> StoreResult<()> {
    match active_generation(conn, target.feed_id)? {
        Some(generation) if generation == target.generation => Ok(()),
        _ => Err(StoreError::NotFound),
    }
}

/// Issues the next ticket of an active subscription; its baseline is the ticket of the version
/// published at this moment.
pub(crate) fn issue_ticket(conn: &Connection, feed_id: i64) -> StoreResult<RefreshTicket> {
    conn.prepare_cached(
        "UPDATE feeds SET refresh_issued = refresh_issued + 1
         WHERE id = ?1 AND is_deleted = 0
         RETURNING refresh_issued, refresh_published",
    )?
    .query_row([feed_id], |row| Ok(RefreshTicket { issued: row.get(0)?, baseline: row.get(1)? }))
    .optional()?
    .ok_or(StoreError::NotFound)
}

/// The commit's ticket; a commit without one is refused (see the module documentation).
fn commit_ticket(target: &RefreshTarget) -> StoreResult<RefreshTicket> {
    if target.ticket == RefreshTicket::UNTRACKED {
        return Err(DomainError::InvalidInput(
            "刷新提交缺少序号：目标必须来自 begin_refresh 或订阅激活".to_owned(),
        )
        .into());
    }
    Ok(target.ticket)
}

/// Published (P) and diagnosed (D) tickets.
fn published_and_diagnosed(conn: &Connection, feed_id: i64) -> StoreResult<(i64, i64)> {
    Ok(conn
        .prepare_cached("SELECT refresh_published, refresh_diagnosed FROM feeds WHERE id = ?1")?
        .query_row([feed_id], |row| Ok((row.get(0)?, row.get(1)?)))?)
}

/// Starts a refresh: issues a ticket and reads the target in the same write transaction.
fn begin_refresh(
    conn: &Connection,
    feed_id: i64,
    expected_generation: Option<i64>,
) -> StoreResult<Option<RefreshTarget>> {
    let Some(row) = feeds::get_active(conn, feed_id)? else { return Ok(None) };
    if expected_generation.is_some_and(|generation| generation != row.generation) {
        return Ok(None);
    }
    let ticket = issue_ticket(conn, feed_id)?;
    let has_entries = has_entries(conn, feed_id)?;
    Ok(Some(RefreshTarget { ticket, ..target_from(row, has_entries) }))
}

/// Records the version published by ticket `ticket` with `validators`. Validators and the last
/// success belong to the published version; the fetch time and the error to the newest
/// diagnosis, so they change only when `ticket >= D`.
fn record_publication(
    conn: &Connection,
    target: &RefreshTarget,
    ticket: i64,
    validators: &RefreshHttpMetadata,
    now: &str,
) -> StoreResult<()> {
    let changed = conn
        .prepare_cached(
            "UPDATE feeds
             SET etag = ?3,
                 last_modified = ?4,
                 last_success_at = ?5,
                 last_fetched_at = CASE WHEN ?6 >= refresh_diagnosed THEN ?5
                                        ELSE last_fetched_at END,
                 fetch_error = CASE WHEN ?6 >= refresh_diagnosed THEN NULL ELSE fetch_error END,
                 refresh_published = ?6,
                 refresh_diagnosed = MAX(refresh_diagnosed, ?6),
                 updated_at = ?5
             WHERE id = ?1 AND generation = ?2 AND is_deleted = 0",
        )?
        .execute(params![
            target.feed_id,
            target.generation,
            validators.etag.as_deref(),
            validators.last_modified.as_deref(),
            now,
            ticket,
        ])?;
    if changed == 0 {
        return Err(StoreError::NotFound);
    }
    Ok(())
}

/// A 304: applied (`true`) only when it confirms exactly the published version. RFC 9110 lets
/// it omit validators; those come from the request, which sent the published version's.
fn record_not_modified(
    conn: &Connection,
    target: &RefreshTarget,
    metadata: &RefreshHttpMetadata,
    now: &str,
) -> StoreResult<bool> {
    ensure_generation(conn, target)?;
    let ticket = commit_ticket(target)?;
    let (published, _) = published_and_diagnosed(conn, target.feed_id)?;
    if ticket.baseline != published {
        return Ok(false);
    }
    let validators = RefreshHttpMetadata {
        etag: metadata.etag.clone().or_else(|| target.etag.clone()),
        last_modified: metadata.last_modified.clone().or_else(|| target.last_modified.clone()),
    };
    record_publication(conn, target, ticket.issued, &validators, now)?;
    Ok(true)
}

/// A failure: recorded (`true`) only when no newer refresh was diagnosed. It stores the
/// response's validators, if any; targets send none while the error stands.
fn record_failure(
    conn: &Connection,
    target: &RefreshTarget,
    failure: &RefreshFailure,
    now: &str,
) -> StoreResult<bool> {
    ensure_generation(conn, target)?;
    let ticket = commit_ticket(target)?;
    let (_, diagnosed) = published_and_diagnosed(conn, target.feed_id)?;
    if ticket.issued <= diagnosed {
        return Ok(false);
    }
    let metadata = failure.metadata.as_ref();
    conn.prepare_cached(
        "UPDATE feeds
         SET etag = ?2, last_modified = ?3, last_fetched_at = ?4, fetch_error = ?5,
             refresh_diagnosed = ?6, updated_at = ?4
         WHERE id = ?1",
    )?
    .execute(params![
        target.feed_id,
        metadata.and_then(|metadata| metadata.etag.as_deref()),
        metadata.and_then(|metadata| metadata.last_modified.as_deref()),
        now,
        failure.message,
        ticket.issued,
    ])?;
    Ok(true)
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
/// entries, or `None` when a newer refresh already published.
fn commit_update(
    conn: &Connection,
    target: &RefreshTarget,
    update: &FeedRefreshUpdate,
    now: &str,
) -> StoreResult<Option<u64>> {
    ensure_generation(conn, target)?;
    let ticket = commit_ticket(target)?;
    let (published, _) = published_and_diagnosed(conn, target.feed_id)?;
    if ticket.issued <= published {
        return Ok(None);
    }
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
    record_publication(conn, target, ticket.issued, &update.metadata, now)?;
    Ok(Some(inserted_count))
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

    /// Whether the failure was recorded (not superseded by a newer diagnosis).
    async fn record_failure(
        &self,
        target: &RefreshTarget,
        failure: RefreshFailure,
    ) -> Result<bool> {
        let target = target.clone();
        self.store
            .write(move |tx| record_failure(tx, &target, &failure, &now()))
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

    /// Hands out the target with a new ticket, as `begin_refresh` does: a target that can be
    /// committed is always a ticketed request.
    async fn get_target(&self, feed_id: i64) -> Result<Option<RefreshTarget>> {
        self.begin_refresh(feed_id, None).await
    }

    async fn begin_refresh(
        &self,
        feed_id: i64,
        expected_generation: Option<i64>,
    ) -> Result<Option<RefreshTarget>> {
        Ok(self
            .store
            .write(move |tx| begin_refresh(tx, feed_id, expected_generation))
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
                let applied = self
                    .store
                    .write(move |tx| record_not_modified(tx, &target, &metadata, &now()))
                    .await
                    .map_err(DomainError::from)
                    .context("更新订阅抓取状态失败")?;
                Ok(RefreshCommitOutcome { inserted_count: 0, superseded: !applied })
            }
            RefreshCommit::Updated { update } => {
                let metadata = update.metadata.clone();
                let owned_target = target.clone();
                let written = self
                    .store
                    .write(move |tx| commit_update(tx, &owned_target, &update, &now()))
                    .await;
                match written {
                    Ok(Some(inserted_count)) => {
                        Ok(RefreshCommitOutcome { inserted_count, superseded: false })
                    }
                    Ok(None) => Ok(RefreshCommitOutcome { inserted_count: 0, superseded: true }),
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
                let recorded = self.record_failure(target, failure).await?;
                Ok(RefreshCommitOutcome { inserted_count: 0, superseded: !recorded })
            }
        }
    }
}
