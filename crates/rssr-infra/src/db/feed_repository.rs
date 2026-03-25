use rssr_domain::{
    DomainError, Feed, FeedRepository, FeedSummary, NewFeedSubscription, Result as DomainResult,
};
use sqlx::Row;
use time::OffsetDateTime;
use url::Url;

use crate::db::SqlitePool;
use crate::parser::ParsedFeed;

#[derive(Clone)]
pub struct SqliteFeedRepository {
    pool: SqlitePool,
}

impl SqliteFeedRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn update_fetch_state(
        &self,
        feed_id: i64,
        etag: Option<&str>,
        last_modified: Option<&str>,
        fetch_error: Option<&str>,
        success: bool,
    ) -> DomainResult<()> {
        let now = now_rfc3339();
        let last_success = success.then_some(now.as_str());

        let result = sqlx::query(
            r#"
            UPDATE feeds
            SET etag = ?2,
                last_modified = ?3,
                last_fetched_at = ?4,
                last_success_at = COALESCE(?5, last_success_at),
                fetch_error = ?6,
                updated_at = ?4
            WHERE id = ?1
            "#,
        )
        .bind(feed_id)
        .bind(etag)
        .bind(last_modified)
        .bind(&now)
        .bind(last_success)
        .bind(fetch_error)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        if result.rows_affected() == 0 {
            return Err(DomainError::NotFound);
        }

        Ok(())
    }

    pub async fn update_feed_metadata(
        &self,
        feed_id: i64,
        parsed_feed: &ParsedFeed,
    ) -> DomainResult<()> {
        let now = now_rfc3339();
        let result = sqlx::query(
            r#"
            UPDATE feeds
            SET title = COALESCE(?2, title),
                site_url = COALESCE(?3, site_url),
                description = COALESCE(?4, description),
                updated_at = ?5
            WHERE id = ?1
            "#,
        )
        .bind(feed_id)
        .bind(parsed_feed.title.as_deref())
        .bind(parsed_feed.site_url.as_ref().map(Url::as_str))
        .bind(parsed_feed.description.as_deref())
        .bind(&now)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        if result.rows_affected() == 0 {
            return Err(DomainError::NotFound);
        }

        Ok(())
    }

    async fn row_to_feed(row: sqlx::sqlite::SqliteRow) -> DomainResult<Feed> {
        Ok(Feed {
            id: row.try_get("id").map_err(map_sqlx_error)?,
            url: parse_url(row.try_get::<String, _>("url").map_err(map_sqlx_error)?)?,
            title: row.try_get("title").map_err(map_sqlx_error)?,
            site_url: parse_optional_url(row.try_get("site_url").map_err(map_sqlx_error)?)?,
            description: row.try_get("description").map_err(map_sqlx_error)?,
            icon_url: parse_optional_url(row.try_get("icon_url").map_err(map_sqlx_error)?)?,
            folder: row.try_get("folder").unwrap_or(None),
            etag: row.try_get("etag").map_err(map_sqlx_error)?,
            last_modified: row.try_get("last_modified").map_err(map_sqlx_error)?,
            last_fetched_at: parse_optional_datetime(
                row.try_get("last_fetched_at").map_err(map_sqlx_error)?,
            )?,
            last_success_at: parse_optional_datetime(
                row.try_get("last_success_at").map_err(map_sqlx_error)?,
            )?,
            fetch_error: row.try_get("fetch_error").map_err(map_sqlx_error)?,
            is_deleted: row.try_get::<i64, _>("is_deleted").map_err(map_sqlx_error)? != 0,
            created_at: parse_datetime(row.try_get("created_at").map_err(map_sqlx_error)?)?,
            updated_at: parse_datetime(row.try_get("updated_at").map_err(map_sqlx_error)?)?,
        })
    }
}

#[async_trait::async_trait]
impl FeedRepository for SqliteFeedRepository {
    async fn upsert_subscription(&self, new_feed: &NewFeedSubscription) -> DomainResult<Feed> {
        let now = now_rfc3339();

        sqlx::query(
            r#"
            INSERT INTO feeds (url, title, folder, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?4)
            ON CONFLICT(url) DO UPDATE SET
                title = CASE
                    WHEN excluded.title IS NULL THEN feeds.title
                    ELSE NULLIF(excluded.title, '')
                END,
                folder = CASE
                    WHEN excluded.folder IS NULL THEN feeds.folder
                    ELSE NULLIF(excluded.folder, '')
                END,
                is_deleted = 0,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(new_feed.url.as_str())
        .bind(new_feed.title.as_deref())
        .bind(new_feed.folder.as_deref())
        .bind(&now)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        let row = sqlx::query("SELECT * FROM feeds WHERE url = ?1")
            .bind(new_feed.url.as_str())
            .fetch_one(&self.pool)
            .await
            .map_err(map_sqlx_error)?;

        Self::row_to_feed(row).await
    }

    async fn set_deleted(&self, feed_id: i64, is_deleted: bool) -> DomainResult<()> {
        let now = now_rfc3339();
        let result = sqlx::query(
            r#"
            UPDATE feeds
            SET is_deleted = ?2,
                updated_at = ?3
            WHERE id = ?1
            "#,
        )
        .bind(feed_id)
        .bind(if is_deleted { 1_i64 } else { 0_i64 })
        .bind(&now)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        if result.rows_affected() == 0 {
            return Err(DomainError::NotFound);
        }

        Ok(())
    }

    async fn list_feeds(&self) -> DomainResult<Vec<Feed>> {
        let rows = sqlx::query(
            "SELECT * FROM feeds WHERE is_deleted = 0 ORDER BY COALESCE(title, url) ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        let mut feeds = Vec::with_capacity(rows.len());
        for row in rows {
            feeds.push(Self::row_to_feed(row).await?);
        }
        Ok(feeds)
    }

    async fn get_feed(&self, feed_id: i64) -> DomainResult<Option<Feed>> {
        let row = sqlx::query("SELECT * FROM feeds WHERE id = ?1 AND is_deleted = 0")
            .bind(feed_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_sqlx_error)?;

        match row {
            Some(row) => Ok(Some(Self::row_to_feed(row).await?)),
            None => Ok(None),
        }
    }

    async fn list_summaries(&self) -> DomainResult<Vec<FeedSummary>> {
        let rows = sqlx::query(
            r#"
            SELECT feeds.id,
                   COALESCE(feeds.title, feeds.url) AS title,
                   COALESCE(SUM(CASE WHEN entries.is_read = 0 THEN 1 ELSE 0 END), 0) AS unread_count
            FROM feeds
            LEFT JOIN entries ON entries.feed_id = feeds.id
            WHERE feeds.is_deleted = 0
            GROUP BY feeds.id, feeds.title, feeds.url
            ORDER BY title ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(rows
            .into_iter()
            .map(|row| FeedSummary {
                id: row.get("id"),
                title: row.get("title"),
                unread_count: row.get::<i64, _>("unread_count") as u32,
            })
            .collect())
    }
}

fn map_sqlx_error(error: sqlx::Error) -> DomainError {
    DomainError::Persistence(error.to_string())
}

fn parse_url(raw: String) -> DomainResult<Url> {
    Url::parse(&raw).map_err(|error| DomainError::Persistence(error.to_string()))
}

fn parse_optional_url(raw: Option<String>) -> DomainResult<Option<Url>> {
    raw.map(parse_url).transpose()
}

fn parse_datetime(raw: String) -> DomainResult<OffsetDateTime> {
    OffsetDateTime::parse(&raw, &time::format_description::well_known::Rfc3339)
        .map_err(|error| DomainError::Persistence(error.to_string()))
}

fn parse_optional_datetime(raw: Option<String>) -> DomainResult<Option<OffsetDateTime>> {
    raw.map(parse_datetime).transpose()
}

fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .expect("format current time")
}
