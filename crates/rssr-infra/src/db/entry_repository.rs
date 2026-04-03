use rssr_domain::{
    DomainError, Entry, EntryNavigation, EntryQuery, EntryRepository, EntrySummary, ReadFilter,
    Result as DomainResult, StarredFilter,
};
use sha2::{Digest, Sha256};
use sqlx::{QueryBuilder, Row, Sqlite};
use time::OffsetDateTime;
use url::Url;

use crate::db::SqlitePool;
use crate::parser::feed_parser::ParsedEntry;

#[derive(Clone)]
pub struct SqliteEntryRepository {
    pool: SqlitePool,
}

#[derive(Debug, Clone)]
pub struct LocalizedEntryUpdate<'a> {
    pub dedup_key: &'a str,
    pub expected_content_hash: &'a str,
    pub localized_html: &'a str,
    pub localized_content_hash: &'a str,
}

impl SqliteEntryRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn upsert_entries(
        &self,
        feed_id: i64,
        entries: &[ParsedEntry],
    ) -> DomainResult<usize> {
        let mut inserted_or_updated = 0;

        for entry in entries {
            let content_hash = hash_content(
                entry.content_html.as_deref(),
                entry.content_text.as_deref(),
                Some(&entry.title),
            );
            let published_at = format_optional_datetime(entry.published_at)?;
            let updated_at_source = format_optional_datetime(entry.updated_at_source)?;
            let now = now_rfc3339();

            let result = sqlx::query(
                r#"
                INSERT INTO entries (
                    feed_id, external_id, dedup_key, url, title, author, summary,
                    content_html, content_text, published_at, updated_at_source,
                    first_seen_at, content_hash, is_read, is_starred, read_at,
                    starred_at, created_at, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, 0, 0, NULL, NULL, ?12, ?12)
                ON CONFLICT(feed_id, dedup_key) DO UPDATE SET
                    external_id = excluded.external_id,
                    url = COALESCE(excluded.url, entries.url),
                    title = excluded.title,
                    author = excluded.author,
                    summary = excluded.summary,
                    content_html = COALESCE(excluded.content_html, entries.content_html),
                    content_text = COALESCE(excluded.content_text, entries.content_text),
                    published_at = COALESCE(excluded.published_at, entries.published_at),
                    updated_at_source = COALESCE(excluded.updated_at_source, entries.updated_at_source),
                    content_hash = excluded.content_hash,
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
            .bind(entry.content_html.as_deref())
            .bind(entry.content_text.as_deref())
            .bind(published_at)
            .bind(updated_at_source)
            .bind(&now)
            .bind(content_hash)
            .execute(&self.pool)
            .await
            .map_err(map_sqlx_error)?;

            if result.rows_affected() > 0 {
                inserted_or_updated += 1;
            }
        }

        Ok(inserted_or_updated)
    }

    pub async fn update_localized_html_if_hash_matches(
        &self,
        feed_id: i64,
        update: &LocalizedEntryUpdate<'_>,
    ) -> DomainResult<bool> {
        let now = now_rfc3339();
        let result = sqlx::query(
            r#"
            UPDATE entries
            SET content_html = ?4,
                content_hash = ?5,
                updated_at = ?6
            WHERE feed_id = ?1
              AND dedup_key = ?2
              AND content_hash = ?3
            "#,
        )
        .bind(feed_id)
        .bind(update.dedup_key)
        .bind(update.expected_content_hash)
        .bind(update.localized_html)
        .bind(update.localized_content_hash)
        .bind(&now)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(result.rows_affected() > 0)
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
            .fetch_optional(&self.pool)
            .await
            .map_err(map_sqlx_error)
            .map(|row| row.map(|row| row.get("id")))
    }

    async fn row_to_entry(row: sqlx::sqlite::SqliteRow) -> DomainResult<Entry> {
        Ok(Entry {
            id: row.try_get("id").map_err(map_sqlx_error)?,
            feed_id: row.try_get("feed_id").map_err(map_sqlx_error)?,
            external_id: row.try_get("external_id").map_err(map_sqlx_error)?,
            dedup_key: row.try_get("dedup_key").map_err(map_sqlx_error)?,
            url: parse_optional_url(row.try_get("url").map_err(map_sqlx_error)?)?,
            title: row.try_get("title").map_err(map_sqlx_error)?,
            author: row.try_get("author").map_err(map_sqlx_error)?,
            summary: row.try_get("summary").map_err(map_sqlx_error)?,
            content_html: row.try_get("content_html").map_err(map_sqlx_error)?,
            content_text: row.try_get("content_text").map_err(map_sqlx_error)?,
            published_at: parse_optional_datetime(
                row.try_get("published_at").map_err(map_sqlx_error)?,
            )?,
            updated_at_source: parse_optional_datetime(
                row.try_get("updated_at_source").map_err(map_sqlx_error)?,
            )?,
            first_seen_at: parse_datetime(row.try_get("first_seen_at").map_err(map_sqlx_error)?)?,
            content_hash: row.try_get("content_hash").map_err(map_sqlx_error)?,
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
            .execute(&self.pool)
            .await
            .map_err(map_sqlx_error)?;

        if result.rows_affected() == 0 {
            return Err(DomainError::NotFound);
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl EntryRepository for SqliteEntryRepository {
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

        qb.push(
            " ORDER BY COALESCE(entries.published_at, entries.created_at) DESC, entries.id DESC",
        );
        if let Some(limit) = query.limit {
            qb.push(" LIMIT ").push_bind(limit as i64);
        }

        let rows = qb.build().fetch_all(&self.pool).await.map_err(map_sqlx_error)?;

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

    async fn get_entry(&self, entry_id: i64) -> DomainResult<Option<Entry>> {
        let row = sqlx::query("SELECT * FROM entries WHERE id = ?1")
            .bind(entry_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_sqlx_error)?;

        match row {
            Some(row) => Ok(Some(Self::row_to_entry(row).await?)),
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
        .fetch_optional(&self.pool)
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
            .execute(&self.pool)
            .await
            .map_err(map_sqlx_error)?;
        Ok(())
    }
}

fn map_sqlx_error(error: sqlx::Error) -> DomainError {
    DomainError::Persistence(error.to_string())
}

fn hash_content(html: Option<&str>, text: Option<&str>, title: Option<&str>) -> Option<String> {
    let mut hasher = Sha256::new();
    let mut used = false;
    for part in [title, text, html].into_iter().flatten() {
        hasher.update(part.as_bytes());
        used = true;
    }
    used.then(|| format!("{:x}", hasher.finalize()))
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
