use rssr_domain::{
    DomainError, Entry, EntryContent, EntryContentRepository, EntryIndexRepository,
    EntryNavigation, EntryQuery, EntryRecord, EntryRepository, EntrySummary, ReadFilter,
    Result as DomainResult, StarredFilter,
};
use sqlx::{QueryBuilder, Row, Sqlite};
use time::OffsetDateTime;
use url::Url;

use crate::db::SqlitePool;
use crate::feed_normalization::hash_content;
use crate::parser::feed_parser::ParsedEntry;

#[derive(Clone)]
pub struct SqliteEntryRepository {
    index_pool: SqlitePool,
    content_pool: SqlitePool,
}

#[derive(Debug, Clone)]
pub struct LocalizedEntryUpdate<'a> {
    pub dedup_key: &'a str,
    pub expected_content_hash: &'a str,
    pub localized_html: &'a str,
    pub localized_content_hash: &'a str,
}

#[derive(Debug, Clone)]
struct PendingEntryContent {
    dedup_key: String,
    content_html: Option<String>,
    content_text: Option<String>,
    content_hash: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ResolvedEntryContent {
    pub entry_id: i64,
    pub dedup_key: String,
    pub content_html: Option<String>,
    pub content_text: Option<String>,
    pub content_hash: Option<String>,
}

impl SqliteEntryRepository {
    pub fn new(index_pool: SqlitePool) -> Self {
        Self::new_with_content_pool(index_pool.clone(), index_pool)
    }

    pub fn new_with_content_pool(index_pool: SqlitePool, content_pool: SqlitePool) -> Self {
        Self { index_pool, content_pool }
    }

    pub async fn upsert_entries(
        &self,
        feed_id: i64,
        entries: &[ParsedEntry],
    ) -> DomainResult<usize> {
        let resolved_contents = self.upsert_entries_and_resolve_contents(feed_id, entries).await?;
        self.upsert_contents(feed_id, &resolved_contents).await?;
        Ok(entries.len())
    }

    pub async fn upsert_entries_and_resolve_contents(
        &self,
        feed_id: i64,
        entries: &[ParsedEntry],
    ) -> DomainResult<Vec<ResolvedEntryContent>> {
        let mut pending_contents = Vec::new();

        for entry in entries {
            let published_at = format_optional_datetime(entry.published_at)?;
            let updated_at_source = format_optional_datetime(entry.updated_at_source)?;
            let now = now_rfc3339();

            sqlx::query(
                r#"
                INSERT INTO entries (
                    feed_id, external_id, dedup_key, url, title, author, summary,
                    published_at, updated_at_source, first_seen_at, has_content, is_read,
                    is_starred, read_at, starred_at, created_at, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 0, 0, 0, NULL, NULL, ?10, ?10)
                ON CONFLICT(feed_id, dedup_key) DO UPDATE SET
                    external_id = excluded.external_id,
                    url = COALESCE(excluded.url, entries.url),
                    title = excluded.title,
                    author = excluded.author,
                    summary = excluded.summary,
                    published_at = COALESCE(excluded.published_at, entries.published_at),
                    updated_at_source = COALESCE(excluded.updated_at_source, entries.updated_at_source),
                    updated_at = excluded.updated_at
                "#,
            )
            .bind(feed_id)
            .bind(&entry.external_id)
            .bind(&entry.dedup_key)
            .bind(entry.url.as_ref().map(Url::as_str))
            .bind(&entry.title)
            .bind(entry.author.as_deref())
            .bind(entry.summary.as_deref())
            .bind(published_at)
            .bind(updated_at_source)
            .bind(&now)
            .execute(&self.index_pool)
            .await
            .map_err(map_sqlx_error)?;

            if entry.content_html.is_some() || entry.content_text.is_some() {
                pending_contents.push(PendingEntryContent {
                    dedup_key: entry.dedup_key.clone(),
                    content_html: entry.content_html.clone(),
                    content_text: entry.content_text.clone(),
                    content_hash: hash_content(
                        entry.content_html.as_deref(),
                        entry.content_text.as_deref(),
                        Some(&entry.title),
                    ),
                });
            }
        }

        if pending_contents.is_empty() {
            return Ok(Vec::new());
        }

        let entry_ids_by_dedup_key = self
            .resolve_entry_ids_by_dedup_keys(
                feed_id,
                &pending_contents
                    .iter()
                    .map(|content| content.dedup_key.as_str())
                    .collect::<Vec<_>>(),
            )
            .await?;

        pending_contents
            .into_iter()
            .map(|content| {
                let entry_id = entry_ids_by_dedup_key
                    .get(content.dedup_key.as_str())
                    .copied()
                    .ok_or(DomainError::NotFound)?;
                Ok(ResolvedEntryContent {
                    entry_id,
                    dedup_key: content.dedup_key,
                    content_html: content.content_html,
                    content_text: content.content_text,
                    content_hash: content.content_hash,
                })
            })
            .collect()
    }

    pub async fn upsert_contents(
        &self,
        feed_id: i64,
        contents: &[ResolvedEntryContent],
    ) -> DomainResult<usize> {
        self.ensure_content_schema().await?;
        let mut upserted = 0;

        for content in contents {
            let now = now_rfc3339();
            let result = sqlx::query(
                r#"
                INSERT INTO entry_contents (
                    entry_id, feed_id, content_html, content_text, content_hash, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                ON CONFLICT(entry_id) DO UPDATE SET
                    feed_id = excluded.feed_id,
                    content_html = COALESCE(excluded.content_html, entry_contents.content_html),
                    content_text = COALESCE(excluded.content_text, entry_contents.content_text),
                    content_hash = excluded.content_hash,
                    updated_at = excluded.updated_at
                "#,
            )
            .bind(content.entry_id)
            .bind(feed_id)
            .bind(content.content_html.as_deref())
            .bind(content.content_text.as_deref())
            .bind(content.content_hash.as_deref())
            .bind(&now)
            .execute(&self.content_pool)
            .await
            .map_err(map_sqlx_error)?;

            if result.rows_affected() > 0 {
                upserted += 1;
            }
        }

        if !contents.is_empty() {
            self.mark_has_content(
                &contents.iter().map(|content| content.entry_id).collect::<Vec<_>>(),
                true,
            )
            .await?;
        }

        Ok(upserted)
    }

    pub async fn update_localized_html_if_hash_matches(
        &self,
        feed_id: i64,
        update: &LocalizedEntryUpdate<'_>,
    ) -> DomainResult<bool> {
        self.ensure_content_schema().await?;
        let entry_id =
            match self.find_entry_id_by_dedup_key_optional(feed_id, update.dedup_key).await? {
                Some(entry_id) => entry_id,
                None => return Ok(false),
            };
        let now = now_rfc3339();
        let result = sqlx::query(
            r#"
            UPDATE entry_contents
            SET content_html = ?2,
                content_hash = ?3,
                updated_at = ?4
            WHERE entry_id = ?1
              AND content_hash = ?5
            "#,
        )
        .bind(entry_id)
        .bind(update.localized_html)
        .bind(update.localized_content_hash)
        .bind(&now)
        .bind(update.expected_content_hash)
        .execute(&self.content_pool)
        .await
        .map_err(map_sqlx_error)?;

        if result.rows_affected() > 0 {
            self.mark_has_content(&[entry_id], true).await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub async fn has_entries_for_feed(&self, feed_id: i64) -> DomainResult<bool> {
        let exists = sqlx::query_scalar::<_, i64>(
            "SELECT EXISTS(SELECT 1 FROM entries WHERE feed_id = ?1 LIMIT 1)",
        )
        .bind(feed_id)
        .fetch_one(&self.index_pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(exists != 0)
    }

    pub async fn list_feed_ids_with_entries(&self) -> DomainResult<std::collections::HashSet<i64>> {
        let rows = sqlx::query_scalar::<_, i64>("SELECT DISTINCT feed_id FROM entries")
            .fetch_all(&self.index_pool)
            .await
            .map_err(map_sqlx_error)?;

        Ok(rows.into_iter().collect())
    }

    pub async fn get_entry(&self, entry_id: i64) -> DomainResult<Option<Entry>> {
        <Self as EntryRepository>::get_entry(self, entry_id).await
    }

    pub async fn list_entries(&self, query: &EntryQuery) -> DomainResult<Vec<EntrySummary>> {
        EntryIndexRepository::list_entries(self, query).await
    }

    pub async fn count_entries(&self, query: &EntryQuery) -> DomainResult<u64> {
        EntryIndexRepository::count_entries(self, query).await
    }

    pub async fn get_entry_record(&self, entry_id: i64) -> DomainResult<Option<EntryRecord>> {
        EntryIndexRepository::get_entry_record(self, entry_id).await
    }

    pub async fn reader_navigation(&self, current_entry_id: i64) -> DomainResult<EntryNavigation> {
        EntryIndexRepository::reader_navigation(self, current_entry_id).await
    }

    pub async fn set_read(&self, entry_id: i64, is_read: bool) -> DomainResult<()> {
        EntryIndexRepository::set_read(self, entry_id, is_read).await
    }

    pub async fn set_starred(&self, entry_id: i64, is_starred: bool) -> DomainResult<()> {
        EntryIndexRepository::set_starred(self, entry_id, is_starred).await
    }

    pub async fn get_content(&self, entry_id: i64) -> DomainResult<Option<EntryContent>> {
        EntryContentRepository::get_content(self, entry_id).await
    }

    async fn mark_has_content(&self, entry_ids: &[i64], has_content: bool) -> DomainResult<()> {
        if entry_ids.is_empty() {
            return Ok(());
        }

        let mut qb = QueryBuilder::<Sqlite>::new("UPDATE entries SET has_content = ");
        qb.push_bind(if has_content { 1_i64 } else { 0_i64 });
        qb.push(" WHERE id IN (");
        let mut separated = qb.separated(", ");
        for entry_id in entry_ids {
            separated.push_bind(entry_id);
        }
        qb.push(")");

        qb.build().execute(&self.index_pool).await.map_err(map_sqlx_error)?;

        Ok(())
    }

    async fn ensure_content_schema(&self) -> DomainResult<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS entry_contents (
                entry_id INTEGER PRIMARY KEY,
                feed_id INTEGER NOT NULL,
                content_html TEXT,
                content_text TEXT,
                content_hash TEXT,
                updated_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&self.content_pool)
        .await
        .map_err(map_sqlx_error)?;
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_entry_contents_feed_id ON entry_contents(feed_id)",
        )
        .execute(&self.content_pool)
        .await
        .map_err(map_sqlx_error)?;
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_entry_contents_updated_at ON entry_contents(updated_at DESC)",
        )
        .execute(&self.content_pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(())
    }

    async fn find_entry_id_by_dedup_key_optional(
        &self,
        feed_id: i64,
        dedup_key: &str,
    ) -> DomainResult<Option<i64>> {
        sqlx::query_scalar::<_, i64>("SELECT id FROM entries WHERE feed_id = ?1 AND dedup_key = ?2")
            .bind(feed_id)
            .bind(dedup_key)
            .fetch_optional(&self.index_pool)
            .await
            .map_err(map_sqlx_error)
    }

    async fn resolve_entry_ids_by_dedup_keys(
        &self,
        feed_id: i64,
        dedup_keys: &[&str],
    ) -> DomainResult<std::collections::HashMap<String, i64>> {
        if dedup_keys.is_empty() {
            return Ok(std::collections::HashMap::new());
        }

        let mut qb =
            QueryBuilder::<Sqlite>::new("SELECT dedup_key, id FROM entries WHERE feed_id = ");
        qb.push_bind(feed_id).push(" AND dedup_key IN (");
        let mut separated = qb.separated(", ");
        for dedup_key in dedup_keys {
            separated.push_bind(dedup_key);
        }
        qb.push(")");

        let rows = qb.build().fetch_all(&self.index_pool).await.map_err(map_sqlx_error)?;
        Ok(rows
            .into_iter()
            .map(|row| (row.get::<String, _>("dedup_key"), row.get::<i64, _>("id")))
            .collect())
    }

    async fn find_adjacent_entry_id(
        &self,
        feed_id: Option<i64>,
        unread_only: bool,
        current_sort_at: &str,
        current_entry_id: i64,
        previous_in_desc_order: bool,
    ) -> DomainResult<Option<i64>> {
        let mut qb = QueryBuilder::<Sqlite>::new(
            r#"
            SELECT entries.id
            FROM entries
            JOIN feeds ON feeds.id = entries.feed_id
            WHERE feeds.is_deleted = 0
            "#,
        );

        if let Some(feed_id) = feed_id {
            qb.push(" AND entries.feed_id = ").push_bind(feed_id);
        }
        if unread_only {
            qb.push(" AND entries.is_read = 0");
        }

        if previous_in_desc_order {
            qb.push(" AND (COALESCE(entries.published_at, entries.created_at) > ")
                .push_bind(current_sort_at)
                .push(" OR (COALESCE(entries.published_at, entries.created_at) = ")
                .push_bind(current_sort_at)
                .push(" AND entries.id > ")
                .push_bind(current_entry_id)
                .push("))")
                .push(
                    " ORDER BY COALESCE(entries.published_at, entries.created_at) ASC, entries.id ASC",
                );
        } else {
            qb.push(" AND (COALESCE(entries.published_at, entries.created_at) < ")
                .push_bind(current_sort_at)
                .push(" OR (COALESCE(entries.published_at, entries.created_at) = ")
                .push_bind(current_sort_at)
                .push(" AND entries.id < ")
                .push_bind(current_entry_id)
                .push("))")
                .push(
                    " ORDER BY COALESCE(entries.published_at, entries.created_at) DESC, entries.id DESC",
                );
        }

        qb.push(" LIMIT 1");

        qb.build()
            .fetch_optional(&self.index_pool)
            .await
            .map_err(map_sqlx_error)
            .map(|row| row.map(|row| row.get("id")))
    }

    async fn row_to_entry_record(row: sqlx::sqlite::SqliteRow) -> DomainResult<EntryRecord> {
        Ok(EntryRecord {
            id: row.try_get("id").map_err(map_sqlx_error)?,
            feed_id: row.try_get("feed_id").map_err(map_sqlx_error)?,
            external_id: row.try_get("external_id").map_err(map_sqlx_error)?,
            dedup_key: row.try_get("dedup_key").map_err(map_sqlx_error)?,
            url: parse_optional_url(row.try_get("url").map_err(map_sqlx_error)?)?,
            title: row.try_get("title").map_err(map_sqlx_error)?,
            author: row.try_get("author").map_err(map_sqlx_error)?,
            summary: row.try_get("summary").map_err(map_sqlx_error)?,
            published_at: parse_optional_datetime(
                row.try_get("published_at").map_err(map_sqlx_error)?,
            )?,
            updated_at_source: parse_optional_datetime(
                row.try_get("updated_at_source").map_err(map_sqlx_error)?,
            )?,
            first_seen_at: parse_datetime(row.try_get("first_seen_at").map_err(map_sqlx_error)?)?,
            has_content: row.try_get::<i64, _>("has_content").map_err(map_sqlx_error)? != 0,
            is_read: row.try_get::<i64, _>("is_read").map_err(map_sqlx_error)? != 0,
            is_starred: row.try_get::<i64, _>("is_starred").map_err(map_sqlx_error)? != 0,
            read_at: parse_optional_datetime(row.try_get("read_at").map_err(map_sqlx_error)?)?,
            starred_at: parse_optional_datetime(
                row.try_get("starred_at").map_err(map_sqlx_error)?,
            )?,
            created_at: parse_datetime(row.try_get("created_at").map_err(map_sqlx_error)?)?,
            updated_at: parse_datetime(row.try_get("updated_at").map_err(map_sqlx_error)?)?,
        })
    }

    async fn row_to_entry_content(row: sqlx::sqlite::SqliteRow) -> DomainResult<EntryContent> {
        Ok(EntryContent {
            entry_id: row.try_get("entry_id").map_err(map_sqlx_error)?,
            content_html: row.try_get("content_html").map_err(map_sqlx_error)?,
            content_text: row.try_get("content_text").map_err(map_sqlx_error)?,
            content_hash: row.try_get("content_hash").map_err(map_sqlx_error)?,
            updated_at: parse_datetime(row.try_get("updated_at").map_err(map_sqlx_error)?)?,
        })
    }

    async fn update_entry_flags(
        &self,
        entry_id: i64,
        field: &str,
        timestamp_field: &str,
        enabled: bool,
    ) -> DomainResult<()> {
        let now = enabled.then(now_rfc3339);
        let sql = format!(
            "UPDATE entries SET {field} = ?2, {timestamp_field} = ?3, updated_at = ?4 WHERE id = ?1"
        );

        let result = sqlx::query(&sql)
            .bind(entry_id)
            .bind(if enabled { 1_i64 } else { 0_i64 })
            .bind(now.as_deref())
            .bind(now_rfc3339())
            .execute(&self.index_pool)
            .await
            .map_err(map_sqlx_error)?;

        if result.rows_affected() == 0 {
            return Err(DomainError::NotFound);
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl EntryIndexRepository for SqliteEntryRepository {
    async fn list_entries(&self, query: &EntryQuery) -> DomainResult<Vec<EntrySummary>> {
        let mut qb = QueryBuilder::<Sqlite>::new(
            r#"
            SELECT entries.id,
                   entries.feed_id,
                   entries.title,
                   COALESCE(feeds.title, feeds.url) AS feed_title,
                   entries.published_at,
                   entries.is_read,
                   entries.is_starred
            FROM entries
            JOIN feeds ON feeds.id = entries.feed_id
            WHERE feeds.is_deleted = 0
            "#,
        );

        push_entry_query_filters(&mut qb, query);

        qb.push(
            " ORDER BY COALESCE(entries.published_at, entries.created_at) DESC, entries.id DESC",
        );
        if let Some(limit) = query.limit {
            qb.push(" LIMIT ").push_bind(limit as i64);
        }

        let rows = qb.build().fetch_all(&self.index_pool).await.map_err(map_sqlx_error)?;

        rows.into_iter()
            .map(|row| {
                Ok(EntrySummary {
                    id: row.get("id"),
                    feed_id: row.get("feed_id"),
                    title: row.get("title"),
                    feed_title: row.get("feed_title"),
                    published_at: parse_optional_datetime(row.get("published_at"))?,
                    is_read: row.get::<i64, _>("is_read") != 0,
                    is_starred: row.get::<i64, _>("is_starred") != 0,
                })
            })
            .collect()
    }

    async fn count_entries(&self, query: &EntryQuery) -> DomainResult<u64> {
        let mut qb = QueryBuilder::<Sqlite>::new(
            r#"
            SELECT COUNT(*) AS count
            FROM entries
            JOIN feeds ON feeds.id = entries.feed_id
            WHERE feeds.is_deleted = 0
            "#,
        );

        push_entry_query_filters(&mut qb, query);

        let row = qb.build().fetch_one(&self.index_pool).await.map_err(map_sqlx_error)?;
        let count: i64 = row.get("count");
        Ok(count as u64)
    }

    async fn get_entry_record(&self, entry_id: i64) -> DomainResult<Option<EntryRecord>> {
        let row = sqlx::query(
            r#"
            SELECT id, feed_id, external_id, dedup_key, url, title, author, summary,
                   published_at, updated_at_source, first_seen_at, has_content, is_read,
                   is_starred, read_at, starred_at, created_at, updated_at
            FROM entries
            WHERE id = ?1
            "#,
        )
        .bind(entry_id)
        .fetch_optional(&self.index_pool)
        .await
        .map_err(map_sqlx_error)?;

        match row {
            Some(row) => Ok(Some(Self::row_to_entry_record(row).await?)),
            None => Ok(None),
        }
    }

    async fn reader_navigation(&self, current_entry_id: i64) -> DomainResult<EntryNavigation> {
        let Some(current_row) = sqlx::query(
            r#"
            SELECT entries.feed_id,
                   COALESCE(entries.published_at, entries.created_at) AS sort_at
            FROM entries
            JOIN feeds ON feeds.id = entries.feed_id
            WHERE entries.id = ?1
              AND feeds.is_deleted = 0
            "#,
        )
        .bind(current_entry_id)
        .fetch_optional(&self.index_pool)
        .await
        .map_err(map_sqlx_error)?
        else {
            return Ok(EntryNavigation::default());
        };

        let feed_id: i64 = current_row.get("feed_id");
        let sort_at: String = current_row.get("sort_at");

        Ok(EntryNavigation {
            previous_unread_entry_id: self
                .find_adjacent_entry_id(None, true, &sort_at, current_entry_id, true)
                .await?,
            next_unread_entry_id: self
                .find_adjacent_entry_id(None, true, &sort_at, current_entry_id, false)
                .await?,
            previous_feed_entry_id: self
                .find_adjacent_entry_id(Some(feed_id), false, &sort_at, current_entry_id, true)
                .await?,
            next_feed_entry_id: self
                .find_adjacent_entry_id(Some(feed_id), false, &sort_at, current_entry_id, false)
                .await?,
        })
    }

    async fn set_read(&self, entry_id: i64, is_read: bool) -> DomainResult<()> {
        self.update_entry_flags(entry_id, "is_read", "read_at", is_read).await
    }

    async fn set_starred(&self, entry_id: i64, is_starred: bool) -> DomainResult<()> {
        self.update_entry_flags(entry_id, "is_starred", "starred_at", is_starred).await
    }

    async fn delete_for_feed(&self, feed_id: i64) -> DomainResult<()> {
        sqlx::query("DELETE FROM entries WHERE feed_id = ?1")
            .bind(feed_id)
            .execute(&self.index_pool)
            .await
            .map_err(map_sqlx_error)?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl EntryContentRepository for SqliteEntryRepository {
    async fn get_content(&self, entry_id: i64) -> DomainResult<Option<EntryContent>> {
        self.ensure_content_schema().await?;
        let row = sqlx::query(
            r#"
            SELECT entry_id, content_html, content_text, content_hash, updated_at
            FROM entry_contents
            WHERE entry_id = ?1
            "#,
        )
        .bind(entry_id)
        .fetch_optional(&self.content_pool)
        .await
        .map_err(map_sqlx_error)?;

        match row {
            Some(row) => Ok(Some(Self::row_to_entry_content(row).await?)),
            None => Ok(None),
        }
    }

    async fn delete_for_feed(&self, feed_id: i64) -> DomainResult<()> {
        self.ensure_content_schema().await?;
        sqlx::query("DELETE FROM entry_contents WHERE feed_id = ?1")
            .bind(feed_id)
            .execute(&self.content_pool)
            .await
            .map_err(map_sqlx_error)?;
        Ok(())
    }

    async fn delete_for_entry_ids(&self, entry_ids: &[i64]) -> DomainResult<()> {
        self.ensure_content_schema().await?;
        if entry_ids.is_empty() {
            return Ok(());
        }

        let mut qb = QueryBuilder::<Sqlite>::new("DELETE FROM entry_contents WHERE entry_id IN (");
        let mut separated = qb.separated(", ");
        for entry_id in entry_ids {
            separated.push_bind(entry_id);
        }
        qb.push(")");
        qb.build().execute(&self.content_pool).await.map_err(map_sqlx_error)?;

        Ok(())
    }
}

fn push_entry_query_filters<'a>(qb: &mut QueryBuilder<'a, Sqlite>, query: &'a EntryQuery) {
    if let Some(feed_id) = query.feed_id {
        qb.push(" AND entries.feed_id = ").push_bind(feed_id);
    }
    if !query.feed_ids.is_empty() {
        qb.push(" AND entries.feed_id IN (");
        let mut separated = qb.separated(", ");
        for feed_id in &query.feed_ids {
            separated.push_bind(feed_id);
        }
        qb.push(")");
    }
    match query.read_filter {
        ReadFilter::All => {}
        ReadFilter::UnreadOnly => {
            qb.push(" AND entries.is_read = 0");
        }
        ReadFilter::ReadOnly => {
            qb.push(" AND entries.is_read = 1");
        }
    }
    match query.starred_filter {
        StarredFilter::All => {}
        StarredFilter::StarredOnly => {
            qb.push(" AND entries.is_starred = 1");
        }
        StarredFilter::UnstarredOnly => {
            qb.push(" AND entries.is_starred = 0");
        }
    }
    if let Some(search) = &query.search_title {
        qb.push(" AND entries.title LIKE ")
            .push_bind(format!("%{search}%"))
            .push(" COLLATE NOCASE");
    }
}

fn map_sqlx_error(error: sqlx::Error) -> DomainError {
    DomainError::Persistence(error.to_string())
}

pub fn compute_entry_content_hash(
    html: Option<&str>,
    text: Option<&str>,
    title: Option<&str>,
) -> Option<String> {
    hash_content(html, text, title)
}

fn format_optional_datetime(value: Option<OffsetDateTime>) -> DomainResult<Option<String>> {
    value
        .map(|value| {
            value
                .format(&time::format_description::well_known::Rfc3339)
                .map_err(|error| DomainError::Persistence(error.to_string()))
        })
        .transpose()
}

fn parse_datetime(raw: String) -> DomainResult<OffsetDateTime> {
    OffsetDateTime::parse(&raw, &time::format_description::well_known::Rfc3339)
        .map_err(|error| DomainError::Persistence(error.to_string()))
}

fn parse_optional_datetime(raw: Option<String>) -> DomainResult<Option<OffsetDateTime>> {
    raw.map(parse_datetime).transpose()
}

fn parse_optional_url(raw: Option<String>) -> DomainResult<Option<Url>> {
    raw.map(|raw| Url::parse(&raw).map_err(|error| DomainError::Persistence(error.to_string())))
        .transpose()
}

fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .expect("format current time")
}
