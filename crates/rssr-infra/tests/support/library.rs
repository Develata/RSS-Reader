//! Test fixture for the v0.2 library store: every port over one in-memory library, plus raw SQL
//! for assertions and failure injection.
#![allow(dead_code)]

use rssr_application::{
    FeedRefreshUpdate, ParsedEntryData, ParsedFeedUpdate, RefreshCommit, RefreshStorePort,
};
use rssr_domain::{Feed, FeedRepository, NewFeedSubscription};
use rssr_infra::store::{
    LibraryAppState, LibraryEntries, LibraryFeeds, LibraryMutations, LibraryQueries,
    LibraryRefreshStore, LibrarySettings, LibraryStore,
};
use time::OffsetDateTime;
use url::Url;

pub struct Library {
    pub store: LibraryStore,
    pub feeds: LibraryFeeds,
    pub entries: LibraryEntries,
    pub refresh: LibraryRefreshStore,
    pub mutations: LibraryMutations,
    pub settings: LibrarySettings,
    pub app_state: LibraryAppState,
    pub queries: LibraryQueries,
}

impl Library {
    pub async fn memory() -> Self {
        Self::over(LibraryStore::open_in_memory().await.expect("open library"))
    }

    pub fn over(store: LibraryStore) -> Self {
        Self {
            feeds: LibraryFeeds::new(store.clone()),
            entries: LibraryEntries::new(store.clone()),
            refresh: LibraryRefreshStore::new(store.clone()),
            mutations: LibraryMutations::new(store.clone()),
            settings: LibrarySettings::new(store.clone()),
            app_state: LibraryAppState::new(store.clone()),
            queries: LibraryQueries::new(store.clone()),
            store,
        }
    }

    pub async fn add_feed(&self, url: &str) -> Feed {
        self.feeds.upsert_subscription(&subscription(url)).await.expect("add feed")
    }

    /// Writes entries the way a refresh does (one commit through the refresh port).
    pub async fn seed(&self, feed_id: i64, entries: Vec<ParsedEntryData>) -> u64 {
        let target = self.refresh.get_target(feed_id).await.expect("target").expect("active feed");
        self.refresh.commit(&target, updated(entries)).await.expect("seed entries").inserted_count
    }

    /// Runs SQL in a write transaction (fixtures, failure-injection triggers).
    pub async fn exec(&self, sql: impl Into<String>) {
        let sql = sql.into();
        self.store.write(move |tx| Ok(tx.execute_batch(&sql)?)).await.expect("exec");
    }

    pub async fn scalar(&self, sql: impl Into<String>) -> i64 {
        let sql = sql.into();
        self.store
            .read(move |conn| Ok(conn.query_row(&sql, [], |row| row.get(0))?))
            .await
            .expect("scalar")
    }

    pub async fn text(&self, sql: impl Into<String>) -> Option<String> {
        let sql = sql.into();
        self.store
            .read(move |conn| Ok(conn.query_row(&sql, [], |row| row.get(0))?))
            .await
            .expect("text")
    }
}

pub fn subscription(url: &str) -> NewFeedSubscription {
    NewFeedSubscription {
        url: Url::parse(url).expect("url"),
        title: None,
        source_title: None,
        site_url: None,
        folder: None,
    }
}

pub fn entry(key: &str) -> ParsedEntryData {
    ParsedEntryData {
        external_id: key.to_owned(),
        dedup_key: key.to_owned(),
        url: None,
        title: key.to_owned(),
        author: None,
        summary: None,
        content_html: None,
        content_text: None,
        published_at: None,
        updated_at_source: None,
    }
}

pub fn published(key: &str, title: &str, at: Option<OffsetDateTime>) -> ParsedEntryData {
    ParsedEntryData { title: title.to_owned(), published_at: at, ..entry(key) }
}

pub fn updated(entries: Vec<ParsedEntryData>) -> RefreshCommit {
    RefreshCommit::Updated {
        update: FeedRefreshUpdate {
            metadata: Default::default(),
            feed: ParsedFeedUpdate { title: None, site_url: None, description: None, entries },
        },
    }
}
