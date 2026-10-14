//! Subscriptions. Synchronous SQL on a connection or transaction, plus the
//! [`FeedRepository`] port over a [`LibraryStore`].

use rssr_domain::{
    ConfigFingerprint, DomainError, Feed, FeedRepository, FeedSummary, FingerprintFeed,
    NewFeedSubscription, Result as DomainResult, UserSettings, normalize_feed_url,
};
use rusqlite::{Connection, OptionalExtension, Row, params};
use url::Url;

use super::codec::{now, parse_optional_time, parse_optional_url, parse_time, parse_url};
use super::documents;
use super::error::{StoreError, StoreResult};
use super::executor::LibraryStore;

/// Columns read by [`feed_from_row`], in its order.
pub(crate) const FEED_COLUMNS: &str = "id, url, custom_title, title, site_url, description, \
     icon_url, folder, etag, last_modified, last_fetched_at, last_success_at, fetch_error, \
     is_deleted, created_at, updated_at, generation";

/// A subscription row with the generation that fences work started for it.
pub(crate) struct FeedRow {
    pub feed: Feed,
    pub generation: i64,
}

pub(crate) fn feed_from_row(row: &Row<'_>) -> StoreResult<FeedRow> {
    let custom_title: Option<String> = row.get(2)?;
    let source_title: Option<String> = row.get(3)?;
    Ok(FeedRow {
        feed: Feed {
            id: row.get(0)?,
            url: parse_url(&row.get::<_, String>(1)?)?,
            title: custom_title.or_else(|| source_title.clone()),
            source_title,
            site_url: parse_optional_url(row.get(4)?)?,
            description: row.get(5)?,
            icon_url: parse_optional_url(row.get(6)?)?,
            folder: row.get(7)?,
            etag: row.get(8)?,
            last_modified: row.get(9)?,
            last_fetched_at: parse_optional_time(row.get(10)?)?,
            last_success_at: parse_optional_time(row.get(11)?)?,
            fetch_error: row.get(12)?,
            is_deleted: row.get(13)?,
            created_at: parse_time(&row.get::<_, String>(14)?)?,
            updated_at: parse_time(&row.get::<_, String>(15)?)?,
        },
        generation: row.get(16)?,
    })
}

/// Active subscriptions ordered by display name.
pub(crate) fn list_active(conn: &Connection) -> StoreResult<Vec<FeedRow>> {
    let mut statement = conn.prepare_cached(&format!(
        "SELECT {FEED_COLUMNS} FROM feeds WHERE is_deleted = 0 \
         ORDER BY COALESCE(custom_title, title, url) ASC"
    ))?;
    let mut rows = statement.query([])?;
    let mut feeds = Vec::new();
    while let Some(row) = rows.next()? {
        feeds.push(feed_from_row(row)?);
    }
    Ok(feeds)
}

pub(crate) fn get_active(conn: &Connection, feed_id: i64) -> StoreResult<Option<FeedRow>> {
    let mut statement = conn.prepare_cached(&format!(
        "SELECT {FEED_COLUMNS} FROM feeds WHERE id = ?1 AND is_deleted = 0"
    ))?;
    let mut rows = statement.query([feed_id])?;
    rows.next()?.map(feed_from_row).transpose()
}

/// Generation of an active subscription, `None` when it is removed or missing.
pub(crate) fn active_generation(conn: &Connection, feed_id: i64) -> StoreResult<Option<i64>> {
    Ok(conn
        .prepare_cached("SELECT generation FROM feeds WHERE id = ?1 AND is_deleted = 0")?
        .query_row([feed_id], |row| row.get(0))
        .optional()?)
}

pub(crate) fn has_active_url(conn: &Connection, normalized_url: &Url) -> StoreResult<bool> {
    Ok(conn
        .prepare_cached("SELECT EXISTS(SELECT 1 FROM feeds WHERE url = ?1 AND is_deleted = 0)")?
        .query_row([normalized_url.as_str()], |row| row.get(0))?)
}

/// Activates or updates one subscription and reads it back.
///
/// Two names are kept apart (audit B6): a chosen name goes to `custom_title` (`None` keeps it,
/// blank clears it, anything else sets it); the probed source title goes to `title` and is only
/// ever filled in, never cleared. `folder` follows the chosen-name rule. Surrounding whitespace
/// is trimmed, as in the browser store and in config import. Reactivating a removed subscription
/// clears what belonged to the old one and bumps the generation.
pub(crate) fn upsert_subscription(
    conn: &Connection,
    new_feed: &NewFeedSubscription,
    now: &str,
) -> StoreResult<FeedRow> {
    let normalized_url = normalize_feed_url(&new_feed.url);
    conn.prepare_cached(
        "INSERT INTO feeds (url, custom_title, title, folder, created_at, updated_at, site_url)
         VALUES (?1, NULLIF(?2, ''), ?3, NULLIF(?4, ''), ?5, ?5, ?6)
         ON CONFLICT(url) DO UPDATE SET
             custom_title = CASE WHEN ?2 IS NULL THEN feeds.custom_title ELSE NULLIF(?2, '') END,
             title = COALESCE(?3, feeds.title),
             folder = CASE WHEN ?4 IS NULL THEN feeds.folder ELSE NULLIF(?4, '') END,
             site_url = CASE
                 WHEN feeds.is_deleted = 1 THEN excluded.site_url
                 ELSE COALESCE(excluded.site_url, feeds.site_url)
             END,
             description = CASE WHEN feeds.is_deleted = 1 THEN NULL ELSE feeds.description END,
             icon_url = CASE WHEN feeds.is_deleted = 1 THEN NULL ELSE feeds.icon_url END,
             etag = CASE WHEN feeds.is_deleted = 1 THEN NULL ELSE feeds.etag END,
             last_modified = CASE WHEN feeds.is_deleted = 1 THEN NULL ELSE feeds.last_modified END,
             last_fetched_at = CASE WHEN feeds.is_deleted = 1 THEN NULL ELSE feeds.last_fetched_at END,
             last_success_at = CASE WHEN feeds.is_deleted = 1 THEN NULL ELSE feeds.last_success_at END,
             fetch_error = CASE WHEN feeds.is_deleted = 1 THEN NULL ELSE feeds.fetch_error END,
             generation = CASE WHEN feeds.is_deleted = 1 THEN feeds.generation + 1
                               ELSE feeds.generation END,
             is_deleted = 0,
             updated_at = excluded.updated_at",
    )?
    .execute(params![
        normalized_url.as_str(),
        new_feed.title.as_deref().map(str::trim),
        new_feed.source_title.as_deref(),
        new_feed.folder.as_deref().map(str::trim),
        now,
        new_feed.site_url.as_ref().map(Url::as_str),
    ])?;
    let mut statement =
        conn.prepare_cached(&format!("SELECT {FEED_COLUMNS} FROM feeds WHERE url = ?1"))?;
    let mut rows = statement.query([normalized_url.as_str()])?;
    let row = rows.next()?.ok_or(StoreError::NotFound)?;
    feed_from_row(row)
}

/// Activates a subscription for the add flow: refuses an address that is already active.
pub(crate) fn activate_new_subscription(
    conn: &Connection,
    new_feed: &NewFeedSubscription,
    now: &str,
) -> StoreResult<FeedRow> {
    let normalized_url = normalize_feed_url(&new_feed.url);
    if has_active_url(conn, &normalized_url)? {
        return Err(DomainError::InvalidInput(format!("该地址已订阅：{normalized_url}")).into());
    }
    upsert_subscription(conn, new_feed, now)
}

/// Marks a subscription removed or active again. Reactivation clears what belonged to the old
/// subscription and bumps the generation.
pub(crate) fn set_deleted(
    conn: &Connection,
    feed_id: i64,
    is_deleted: bool,
    now: &str,
) -> StoreResult<()> {
    let changed = conn
        .prepare_cached(
            "UPDATE feeds
             SET generation = CASE WHEN is_deleted = 1 AND ?2 = 0 THEN generation + 1
                                   ELSE generation END,
                 site_url = CASE WHEN is_deleted = 1 AND ?2 = 0 THEN NULL ELSE site_url END,
                 description = CASE WHEN is_deleted = 1 AND ?2 = 0 THEN NULL ELSE description END,
                 icon_url = CASE WHEN is_deleted = 1 AND ?2 = 0 THEN NULL ELSE icon_url END,
                 etag = CASE WHEN is_deleted = 1 AND ?2 = 0 THEN NULL ELSE etag END,
                 last_modified = CASE WHEN is_deleted = 1 AND ?2 = 0 THEN NULL ELSE last_modified END,
                 last_fetched_at = CASE WHEN is_deleted = 1 AND ?2 = 0 THEN NULL ELSE last_fetched_at END,
                 last_success_at = CASE WHEN is_deleted = 1 AND ?2 = 0 THEN NULL ELSE last_success_at END,
                 fetch_error = CASE WHEN is_deleted = 1 AND ?2 = 0 THEN NULL ELSE fetch_error END,
                 is_deleted = ?2,
                 updated_at = ?3
             WHERE id = ?1",
        )?
        .execute(params![feed_id, is_deleted, now])?;
    if changed == 0 {
        return Err(StoreError::NotFound);
    }
    Ok(())
}

/// An active subscription's configuration fields, raw: the custom name as stored (never the
/// source title) and the generation.
pub(crate) struct ConfigRow {
    pub id: i64,
    pub url: String,
    pub generation: i64,
    pub custom_title: Option<String>,
    pub folder: Option<String>,
}

pub(crate) fn config_rows(conn: &Connection) -> StoreResult<Vec<ConfigRow>> {
    let mut statement = conn.prepare_cached(
        "SELECT id, url, generation, custom_title, folder FROM feeds WHERE is_deleted = 0",
    )?;
    let mut rows = statement.query([])?;
    let mut config = Vec::new();
    while let Some(row) = rows.next()? {
        config.push(ConfigRow {
            id: row.get(0)?,
            url: row.get(1)?,
            generation: row.get(2)?,
            custom_title: row.get(3)?,
            folder: row.get(4)?,
        });
    }
    Ok(config)
}

pub(crate) fn fingerprint_of(rows: &[ConfigRow], settings: &UserSettings) -> ConfigFingerprint {
    let projection = rows
        .iter()
        .map(|row| FingerprintFeed {
            id: row.id,
            url: &row.url,
            generation: row.generation,
            custom_title: row.custom_title.as_deref(),
            folder: row.folder.as_deref(),
        })
        .collect();
    ConfigFingerprint::compute(projection, settings)
}

/// The fingerprint of the configuration as this transaction sees it.
pub(crate) fn config_fingerprint(conn: &Connection) -> StoreResult<ConfigFingerprint> {
    let settings: UserSettings = documents::load(conn, documents::USER_SETTINGS_KEY)?;
    Ok(fingerprint_of(&config_rows(conn)?, &settings))
}

pub(crate) fn list_summaries(conn: &Connection) -> StoreResult<Vec<FeedSummary>> {
    let mut statement = conn.prepare_cached(
        "SELECT feeds.id,
                COALESCE(feeds.custom_title, feeds.title, feeds.url) AS display_title,
                feeds.url,
                COALESCE(SUM(CASE WHEN entries.is_read = 0 THEN 1 ELSE 0 END), 0),
                COUNT(entries.id),
                feeds.last_fetched_at,
                feeds.last_success_at,
                feeds.fetch_error
         FROM feeds
         LEFT JOIN entries ON entries.feed_id = feeds.id
         WHERE feeds.is_deleted = 0
         GROUP BY feeds.id
         ORDER BY display_title ASC",
    )?;
    let mut rows = statement.query([])?;
    let mut summaries = Vec::new();
    while let Some(row) = rows.next()? {
        summaries.push(FeedSummary {
            id: row.get(0)?,
            title: row.get(1)?,
            url: row.get(2)?,
            unread_count: row.get(3)?,
            entry_count: row.get(4)?,
            // A summary is for display: an unreadable time shows as unknown.
            last_fetched_at: parse_optional_time(row.get(5)?).ok().flatten(),
            last_success_at: parse_optional_time(row.get(6)?).ok().flatten(),
            fetch_error: row.get(7)?,
        });
    }
    Ok(summaries)
}

/// [`FeedRepository`] over the library.
#[derive(Clone)]
pub struct LibraryFeeds {
    store: LibraryStore,
}

impl LibraryFeeds {
    pub fn new(store: LibraryStore) -> Self {
        Self { store }
    }
}

#[async_trait::async_trait]
impl FeedRepository for LibraryFeeds {
    async fn upsert_subscription(&self, new_feed: &NewFeedSubscription) -> DomainResult<Feed> {
        let new_feed = new_feed.clone();
        let row = self.store.write(move |tx| upsert_subscription(tx, &new_feed, &now())).await?;
        Ok(row.feed)
    }

    async fn upsert_subscriptions(
        &self,
        new_feeds: &[NewFeedSubscription],
    ) -> DomainResult<Vec<Feed>> {
        if new_feeds.is_empty() {
            return Ok(Vec::new());
        }
        let new_feeds = new_feeds.to_vec();
        Ok(self
            .store
            .write(move |tx| {
                let now = now();
                new_feeds
                    .iter()
                    .map(|new_feed| Ok(upsert_subscription(tx, new_feed, &now)?.feed))
                    .collect()
            })
            .await?)
    }

    async fn has_active_subscription_url(&self, url: &Url) -> DomainResult<bool> {
        let normalized_url = normalize_feed_url(url);
        Ok(self.store.read(move |conn| has_active_url(conn, &normalized_url)).await?)
    }

    async fn set_deleted(&self, feed_id: i64, is_deleted: bool) -> DomainResult<()> {
        Ok(self.store.write(move |tx| set_deleted(tx, feed_id, is_deleted, &now())).await?)
    }

    async fn list_feeds(&self) -> DomainResult<Vec<Feed>> {
        let rows = self.store.read(list_active).await?;
        Ok(rows.into_iter().map(|row| row.feed).collect())
    }

    async fn get_feed(&self, feed_id: i64) -> DomainResult<Option<Feed>> {
        let row = self.store.read(move |conn| get_active(conn, feed_id)).await?;
        Ok(row.map(|row| row.feed))
    }

    async fn list_summaries(&self) -> DomainResult<Vec<FeedSummary>> {
        Ok(self.store.read(list_summaries).await?)
    }
}
