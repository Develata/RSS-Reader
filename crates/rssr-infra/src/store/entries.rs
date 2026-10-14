//! Entries and bodies: list queries, reader navigation, read/starred flags and bulk mark-read,
//! plus the [`EntryIndexRepository`] / [`EntryContentRepository`] ports over a [`LibraryStore`].
//!
//! Lists, counts, bulk mark-read and reader navigation share one ordering,
//! `COALESCE(published_at, created_at) DESC, id DESC`, and one filter builder, so the four agree
//! on which entries exist and in which order (see `db/AGENTS.md`).

use std::sync::Arc;

use rssr_domain::{
    ArchiveFilter, Entry, EntryContent, EntryContentRepository, EntryIndexRepository,
    EntryNavigation, EntryQuery, EntryRecord, EntrySummary, MarkReadOutcome, MarkReadPreview,
    ReadFilter, Result as DomainResult, StarredFilter,
};
use rusqlite::types::Value;
use rusqlite::{Connection, OptionalExtension, params, params_from_iter};

use super::codec::{format_time, now, parse_optional_time, parse_optional_url, parse_time};
use super::error::{StoreError, StoreResult};
use super::executor::LibraryStore;

const SORT_KEY: &str = "COALESCE(entries.published_at, entries.created_at)";

/// SQL text plus its positional parameters (`?` placeholders, in order).
struct Filtered {
    sql: String,
    params: Vec<Value>,
}

impl Filtered {
    fn new(head: &str) -> Self {
        Self { sql: head.to_owned(), params: Vec::new() }
    }

    fn push(&mut self, sql: &str) -> &mut Self {
        self.sql.push_str(sql);
        self
    }

    fn bind(&mut self, value: impl Into<Value>) -> &mut Self {
        self.sql.push('?');
        self.params.push(value.into());
        self
    }

    /// Appends the conditions of `query` (a `WHERE` clause must already be open).
    ///
    /// Feed scopes go through one JSON parameter (`json_each`) whatever their size: one
    /// statement shape for every scope keeps the prepared-statement cache effective and stays
    /// clear of SQLite's parameter limit.
    fn entry_filters(&mut self, query: &EntryQuery) -> StoreResult<&mut Self> {
        if let Some(feed_id) = query.feed_id {
            self.push(" AND entries.feed_id = ").bind(feed_id);
        }
        if !query.feed_ids.is_empty() {
            let encoded = serde_json::to_string(&query.feed_ids).unwrap_or_default();
            self.push(" AND entries.feed_id IN (SELECT value FROM json_each(")
                .bind(encoded)
                .push("))");
        }
        match query.read_filter {
            ReadFilter::All => {}
            ReadFilter::UnreadOnly => {
                self.push(" AND entries.is_read = 0");
            }
            ReadFilter::ReadOnly => {
                self.push(" AND entries.is_read = 1");
            }
        }
        match query.starred_filter {
            StarredFilter::All => {}
            StarredFilter::StarredOnly => {
                self.push(" AND entries.is_starred = 1");
            }
            StarredFilter::UnstarredOnly => {
                self.push(" AND entries.is_starred = 0");
            }
        }
        // Archiving looks at published_at only; entries without one are never archived.
        match query.archive_filter {
            ArchiveFilter::All => {}
            ArchiveFilter::ExcludeArchived { cutoff } => {
                self.push(" AND (entries.published_at IS NULL OR entries.published_at >= ")
                    .bind(format_time(cutoff)?)
                    .push(")");
            }
            ArchiveFilter::OnlyArchived { cutoff } => {
                self.push(" AND entries.published_at IS NOT NULL AND entries.published_at < ")
                    .bind(format_time(cutoff)?);
            }
        }
        // Literal substring of the folded title (`%` and `_` are not wildcards), matching the
        // browser store's `contains`; `title_search` holds `fold_for_search(title)`.
        if let Some(search) = &query.search_title {
            self.push(" AND instr(entries.title_search, ")
                .bind(rssr_domain::fold_for_search(search))
                .push(") > 0");
        }
        Ok(self)
    }
}

const ACTIVE_ENTRIES: &str =
    " FROM entries JOIN feeds ON feeds.id = entries.feed_id WHERE feeds.is_deleted = 0";

pub(crate) fn list_entries(
    conn: &Connection,
    query: &EntryQuery,
) -> StoreResult<Vec<EntrySummary>> {
    let mut sql = Filtered::new(
        "SELECT entries.id, entries.feed_id, entries.title,
                COALESCE(feeds.custom_title, feeds.title, feeds.url),
                entries.published_at, entries.is_read, entries.is_starred",
    );
    sql.push(ACTIVE_ENTRIES).entry_filters(query)?;
    sql.push(" ORDER BY ").push(SORT_KEY).push(" DESC, entries.id DESC");
    if let Some(limit) = query.limit {
        sql.push(" LIMIT ").bind(i64::from(limit));
    }
    let mut statement = conn.prepare_cached(&sql.sql)?;
    let mut rows = statement.query(params_from_iter(&sql.params))?;
    let mut entries = Vec::new();
    while let Some(row) = rows.next()? {
        entries.push(EntrySummary {
            id: row.get(0)?,
            feed_id: row.get(1)?,
            title: row.get(2)?,
            feed_title: row.get(3)?,
            published_at: parse_optional_time(row.get(4)?)?,
            is_read: row.get(5)?,
            is_starred: row.get(6)?,
        });
    }
    Ok(entries)
}

pub(crate) fn count_entries(conn: &Connection, query: &EntryQuery) -> StoreResult<u64> {
    let mut sql = Filtered::new("SELECT COUNT(*)");
    sql.push(ACTIVE_ENTRIES).entry_filters(query)?;
    let count: i64 = conn
        .prepare_cached(&sql.sql)?
        .query_row(params_from_iter(&sql.params), |row| row.get(0))?;
    Ok(count as u64)
}

/// Unread entries matching `query` (its limit ignored), in id order.
fn unread_selection(conn: &Connection, query: &EntryQuery) -> StoreResult<Vec<i64>> {
    let mut sql = Filtered::new("SELECT entries.id");
    sql.push(ACTIVE_ENTRIES).push(" AND entries.is_read = 0").entry_filters(query)?;
    sql.push(" ORDER BY entries.id");
    let mut statement = conn.prepare_cached(&sql.sql)?;
    let ids = statement
        .query_map(params_from_iter(&sql.params), |row| row.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(ids)
}

fn without_limit(query: &EntryQuery) -> EntryQuery {
    EntryQuery { limit: None, ..query.clone() }
}

pub(crate) fn preview_mark_read(
    conn: &Connection,
    query: &EntryQuery,
) -> StoreResult<MarkReadPreview> {
    let query = without_limit(query);
    let unread_entry_ids = unread_selection(conn, &query)?;
    Ok(MarkReadPreview { query, unread_entry_ids })
}

/// Marks the previewed entries read if the selection is still exactly the previewed one; the
/// comparison and the update run in the caller's write transaction.
pub(crate) fn mark_read_if_unchanged(
    conn: &Connection,
    preview: &MarkReadPreview,
    now: &str,
) -> StoreResult<MarkReadOutcome> {
    let query = without_limit(&preview.query);
    let unread_entry_ids = unread_selection(conn, &query)?;
    if unread_entry_ids != preview.unread_entry_ids {
        return Ok(MarkReadOutcome::SelectionChanged {
            preview: MarkReadPreview { query, unread_entry_ids },
        });
    }
    if unread_entry_ids.is_empty() {
        return Ok(MarkReadOutcome::Applied { changed_count: 0 });
    }
    let mut sql = Filtered::new("UPDATE entries SET is_read = 1, read_at = ");
    sql.bind(now.to_owned()).push(", updated_at = ").bind(now.to_owned());
    sql.push(" WHERE id IN (SELECT entries.id")
        .push(ACTIVE_ENTRIES)
        .push(" AND entries.is_read = 0");
    sql.entry_filters(&query)?.push(")");
    let changed = conn.prepare_cached(&sql.sql)?.execute(params_from_iter(&sql.params))?;
    Ok(MarkReadOutcome::Applied { changed_count: changed as u64 })
}

const RECORD_COLUMNS: &str = "id, feed_id, external_id, dedup_key, url, title, author, summary, \
     published_at, updated_at_source, first_seen_at, has_content, is_read, is_starred, \
     read_at, starred_at, created_at, updated_at";

pub(crate) fn get_entry_record(
    conn: &Connection,
    entry_id: i64,
) -> StoreResult<Option<EntryRecord>> {
    let mut statement =
        conn.prepare_cached(&format!("SELECT {RECORD_COLUMNS} FROM entries WHERE id = ?1"))?;
    let mut rows = statement.query([entry_id])?;
    let Some(row) = rows.next()? else { return Ok(None) };
    Ok(Some(EntryRecord {
        id: row.get(0)?,
        feed_id: row.get(1)?,
        external_id: row.get(2)?,
        dedup_key: row.get(3)?,
        url: parse_optional_url(row.get(4)?)?,
        title: row.get(5)?,
        author: row.get(6)?,
        summary: row.get(7)?,
        published_at: parse_optional_time(row.get(8)?)?,
        updated_at_source: parse_optional_time(row.get(9)?)?,
        first_seen_at: parse_time(&row.get::<_, String>(10)?)?,
        has_content: row.get(11)?,
        is_read: row.get(12)?,
        is_starred: row.get(13)?,
        read_at: parse_optional_time(row.get(14)?)?,
        starred_at: parse_optional_time(row.get(15)?)?,
        created_at: parse_time(&row.get::<_, String>(16)?)?,
        updated_at: parse_time(&row.get::<_, String>(17)?)?,
    }))
}

pub(crate) fn get_content(conn: &Connection, entry_id: i64) -> StoreResult<Option<EntryContent>> {
    let row = conn
        .prepare_cached(
            "SELECT content_html, content_text, content_hash, updated_at
             FROM entry_contents WHERE entry_id = ?1",
        )?
        .query_row([entry_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get::<_, String>(3)?))
        })
        .optional()?;
    row.map(|(content_html, content_text, content_hash, updated_at)| {
        Ok(EntryContent {
            entry_id,
            content_html,
            content_text,
            content_hash,
            updated_at: parse_time(&updated_at)?,
        })
    })
    .transpose()
}

/// The adjacent entry in list order: `towards_newer` looks above `current` in the descending
/// list, otherwise below it. Each variant seeks along one sort-key index.
fn adjacent_entry(
    conn: &Connection,
    feed_id: Option<i64>,
    unread_only: bool,
    sort_at: &str,
    current_entry_id: i64,
    towards_newer: bool,
) -> StoreResult<Option<i64>> {
    let mut sql = Filtered::new("SELECT entries.id");
    sql.push(ACTIVE_ENTRIES);
    if let Some(feed_id) = feed_id {
        sql.push(" AND entries.feed_id = ").bind(feed_id);
    }
    if unread_only {
        sql.push(" AND entries.is_read = 0");
    }
    let (compare, order) = if towards_newer { (">", "ASC") } else { ("<", "DESC") };
    sql.push(" AND (").push(SORT_KEY).push(" ").push(compare).push(" ").bind(sort_at.to_owned());
    sql.push(" OR (").push(SORT_KEY).push(" = ").bind(sort_at.to_owned());
    sql.push(" AND entries.id ").push(compare).push(" ").bind(current_entry_id).push("))");
    sql.push(" ORDER BY ").push(SORT_KEY).push(" ").push(order);
    sql.push(", entries.id ").push(order).push(" LIMIT 1");
    Ok(conn
        .prepare_cached(&sql.sql)?
        .query_row(params_from_iter(&sql.params), |row| row.get(0))
        .optional()?)
}

/// All four neighbours from one snapshot (the caller's read transaction).
pub(crate) fn reader_navigation(
    conn: &Connection,
    current_entry_id: i64,
) -> StoreResult<EntryNavigation> {
    let current = conn
        .prepare_cached(
            "SELECT entries.feed_id, COALESCE(entries.published_at, entries.created_at)
             FROM entries JOIN feeds ON feeds.id = entries.feed_id
             WHERE entries.id = ?1 AND feeds.is_deleted = 0",
        )?
        .query_row([current_entry_id], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)))
        .optional()?;
    let Some((feed_id, sort_at)) = current else {
        return Ok(EntryNavigation::default());
    };
    Ok(EntryNavigation {
        previous_unread_entry_id: adjacent_entry(
            conn,
            None,
            true,
            &sort_at,
            current_entry_id,
            true,
        )?,
        next_unread_entry_id: adjacent_entry(conn, None, true, &sort_at, current_entry_id, false)?,
        previous_feed_entry_id: adjacent_entry(
            conn,
            Some(feed_id),
            false,
            &sort_at,
            current_entry_id,
            true,
        )?,
        next_feed_entry_id: adjacent_entry(
            conn,
            Some(feed_id),
            false,
            &sort_at,
            current_entry_id,
            false,
        )?,
    })
}

/// The two entry flags, each with its own fixed statement.
#[derive(Clone, Copy)]
pub(crate) enum EntryFlag {
    Read,
    Starred,
}

impl EntryFlag {
    fn update_sql(self) -> &'static str {
        match self {
            Self::Read => {
                "UPDATE entries SET is_read = ?2, read_at = ?3, updated_at = ?4
                 WHERE id = ?1 AND is_read <> ?2"
            }
            Self::Starred => {
                "UPDATE entries SET is_starred = ?2, starred_at = ?3, updated_at = ?4
                 WHERE id = ?1 AND is_starred <> ?2"
            }
        }
    }
}

/// Sets the flag to `enabled`; `true` when it changed. The row (and its timestamps) is only
/// written on a change, so repeating a request keeps the original time.
pub(crate) fn set_flag(
    conn: &Connection,
    entry_id: i64,
    flag: EntryFlag,
    enabled: bool,
    now: &str,
) -> StoreResult<bool> {
    let changed = conn.prepare_cached(flag.update_sql())?.execute(params![
        entry_id,
        enabled,
        enabled.then_some(now),
        now
    ])?;
    if changed > 0 {
        return Ok(true);
    }
    let exists: bool = conn
        .prepare_cached("SELECT EXISTS (SELECT 1 FROM entries WHERE id = ?1)")?
        .query_row([entry_id], |row| row.get(0))?;
    if exists { Ok(false) } else { Err(StoreError::NotFound) }
}

/// Deletes a subscription's entries; their bodies go with them (foreign-key cascade).
pub(crate) fn delete_for_feed(conn: &Connection, feed_id: i64) -> StoreResult<()> {
    conn.prepare_cached("DELETE FROM entries WHERE feed_id = ?1")?.execute([feed_id])?;
    Ok(())
}

pub(crate) fn delete_contents(conn: &Connection, entry_ids: &[i64]) -> StoreResult<()> {
    if entry_ids.is_empty() {
        return Ok(());
    }
    let encoded = serde_json::to_string(entry_ids).unwrap_or_default();
    conn.prepare_cached(
        "DELETE FROM entry_contents WHERE entry_id IN (SELECT value FROM json_each(?1))",
    )?
    .execute([encoded])?;
    Ok(())
}

/// [`EntryIndexRepository`] and [`EntryContentRepository`] over the library.
#[derive(Clone)]
pub struct LibraryEntries {
    pub(crate) store: LibraryStore,
}

impl LibraryEntries {
    pub fn new(store: LibraryStore) -> Self {
        Self { store }
    }
}

#[async_trait::async_trait]
impl EntryIndexRepository for LibraryEntries {
    async fn preview_mark_read(&self, query: &EntryQuery) -> DomainResult<MarkReadPreview> {
        let query = query.clone();
        Ok(self.store.read(move |conn| preview_mark_read(conn, &query)).await?)
    }

    async fn mark_read_if_unchanged(
        &self,
        preview: &MarkReadPreview,
    ) -> DomainResult<MarkReadOutcome> {
        let preview = preview.clone();
        Ok(self.store.write(move |tx| mark_read_if_unchanged(tx, &preview, &now())).await?)
    }

    async fn list_entries(&self, query: &EntryQuery) -> DomainResult<Vec<EntrySummary>> {
        let query = query.clone();
        Ok(self.store.read(move |conn| list_entries(conn, &query)).await?)
    }

    async fn count_entries(&self, query: &EntryQuery) -> DomainResult<u64> {
        let query = query.clone();
        Ok(self.store.read(move |conn| count_entries(conn, &query)).await?)
    }

    async fn get_entry_record(&self, entry_id: i64) -> DomainResult<Option<EntryRecord>> {
        Ok(self.store.read(move |conn| get_entry_record(conn, entry_id)).await?)
    }

    async fn reader_navigation(&self, current_entry_id: i64) -> DomainResult<EntryNavigation> {
        Ok(self.store.read(move |conn| reader_navigation(conn, current_entry_id)).await?)
    }

    async fn set_read(&self, entry_id: i64, is_read: bool) -> DomainResult<bool> {
        Ok(self
            .store
            .write(move |tx| set_flag(tx, entry_id, EntryFlag::Read, is_read, &now()))
            .await?)
    }

    async fn set_starred(&self, entry_id: i64, is_starred: bool) -> DomainResult<bool> {
        Ok(self
            .store
            .write(move |tx| set_flag(tx, entry_id, EntryFlag::Starred, is_starred, &now()))
            .await?)
    }

    async fn delete_for_feed(&self, feed_id: i64) -> DomainResult<()> {
        Ok(self.store.write(move |tx| delete_for_feed(tx, feed_id)).await?)
    }
}

#[async_trait::async_trait]
impl EntryContentRepository for LibraryEntries {
    async fn get_content(&self, entry_id: i64) -> DomainResult<Option<EntryContent>> {
        Ok(self.store.read(move |conn| get_content(conn, entry_id)).await?)
    }

    /// Bodies are removed with their entries; only the bodies of entries that remain are
    /// deleted here.
    async fn delete_for_feed(&self, feed_id: i64) -> DomainResult<()> {
        Ok(self
            .store
            .write(move |tx| {
                tx.prepare_cached(
                    "DELETE FROM entry_contents
                     WHERE entry_id IN (SELECT id FROM entries WHERE feed_id = ?1)",
                )?
                .execute([feed_id])?;
                Ok(())
            })
            .await?)
    }

    async fn delete_for_entry_ids(&self, entry_ids: &[i64]) -> DomainResult<()> {
        if entry_ids.is_empty() {
            return Ok(());
        }
        let entry_ids = entry_ids.to_vec();
        Ok(self.store.write(move |tx| delete_contents(tx, &entry_ids)).await?)
    }
}

/// The source-body hash stored with a refreshed body (`entry_contents.content_hash`).
pub fn compute_entry_content_hash(
    html: Option<&str>,
    text: Option<&str>,
    title: Option<&str>,
    url: Option<&url::Url>,
) -> Option<String> {
    crate::feed_normalization::hash_content(html, text, title, url.map(url::Url::as_str))
}

/// A localized body to write back over the body it was derived from.
///
/// `expected_content_hash` is the source hash the derivation started from, compared with `IS`
/// (rows from old versions may hold NULL). `base_html` is the stored body the derivation read:
/// the write happens only while the store still holds exactly it, so of two localizations of the
/// same source (background and reader) the slower one cannot overwrite the first.
#[derive(Debug, Clone)]
pub struct LocalizedBody {
    pub dedup_key: String,
    pub expected_content_hash: Option<String>,
    pub base_html: Arc<str>,
    pub localized_html: String,
}

fn entry_id_for(conn: &Connection, feed_id: i64, dedup_key: &str) -> StoreResult<Option<i64>> {
    Ok(conn
        .prepare_cached("SELECT id FROM entries WHERE feed_id = ?1 AND dedup_key = ?2")?
        .query_row(params![feed_id, dedup_key], |row| row.get(0))
        .optional()?)
}

fn update_localized_body(
    conn: &Connection,
    feed_id: i64,
    generation: i64,
    body: &LocalizedBody,
    now: &str,
) -> StoreResult<bool> {
    if super::feeds::active_generation(conn, feed_id)? != Some(generation) {
        return Ok(false);
    }
    let Some(entry_id) = entry_id_for(conn, feed_id, &body.dedup_key)? else {
        return Ok(false);
    };
    let changed = conn
        .prepare_cached(
            "UPDATE entry_contents SET content_html = ?2, updated_at = ?3
             WHERE entry_id = ?1 AND content_hash IS ?4 AND content_html = ?5",
        )?
        .execute(params![
            entry_id,
            body.localized_html,
            now,
            body.expected_content_hash,
            &*body.base_html
        ])?;
    if changed == 0 {
        return Ok(false);
    }
    conn.prepare_cached("UPDATE entries SET has_content = 1 WHERE id = ?1")?.execute([entry_id])?;
    Ok(true)
}

/// Body localization, reached from the GUI's image localization worker.
impl LibraryEntries {
    /// The subscription and generation of an entry whose subscription is active.
    pub async fn active_entry_generation(&self, entry_id: i64) -> DomainResult<Option<(i64, i64)>> {
        Ok(self
            .store
            .read(move |conn| {
                Ok(conn
                    .prepare_cached(
                        "SELECT entries.feed_id, feeds.generation
                         FROM entries JOIN feeds ON feeds.id = entries.feed_id
                         WHERE entries.id = ?1 AND feeds.is_deleted = 0",
                    )?
                    .query_row([entry_id], |row| Ok((row.get(0)?, row.get(1)?)))
                    .optional()?)
            })
            .await?)
    }

    /// The entry with its body, read from one snapshot.
    pub async fn get_entry(&self, entry_id: i64) -> DomainResult<Option<Entry>> {
        Ok(self
            .store
            .read(move |conn| {
                let Some(record) = get_entry_record(conn, entry_id)? else { return Ok(None) };
                Ok(Some(record.into_entry(get_content(conn, entry_id)?)))
            })
            .await?)
    }

    /// Whether the stored body is already derived from this source: same source hash, other
    /// HTML. A derived body is written only when at least one image was localized, so the
    /// background localizer skips such entries instead of downloading the images again (C1).
    pub async fn has_derived_body(
        &self,
        feed_id: i64,
        dedup_key: String,
        source_hash: String,
        source_html: Arc<str>,
    ) -> DomainResult<bool> {
        Ok(self
            .store
            .read(move |conn| {
                Ok(conn
                    .prepare_cached(
                        "SELECT EXISTS(
                             SELECT 1 FROM entries
                             JOIN entry_contents ON entry_contents.entry_id = entries.id
                             WHERE entries.feed_id = ?1 AND entries.dedup_key = ?2
                               AND entry_contents.content_hash = ?3
                               AND entry_contents.content_html <> ?4)",
                    )?
                    .query_row(params![feed_id, dedup_key, source_hash, &*source_html], |row| {
                        row.get(0)
                    })?)
            })
            .await?)
    }

    /// Writes `body` if the subscription is still in `generation` and the stored body is still
    /// the one it was derived from; returns whether it was written.
    pub async fn update_localized_body(
        &self,
        feed_id: i64,
        generation: i64,
        body: LocalizedBody,
    ) -> DomainResult<bool> {
        Ok(self
            .store
            .write(move |tx| update_localized_body(tx, feed_id, generation, &body, &now()))
            .await?)
    }
}
