//! Reads the user interface needs as one consistent snapshot of the library: each method is
//! answered inside one read transaction of the store (`docs/design/v0.2-architecture.md`, 架构:
//! 聚合查询端口保证读快照). Without it, a removal or refresh by another program between two
//! reads could mix states, for example an entry with the navigation of a newer list.

use rssr_domain::{
    ConfigFingerprint, EntriesWorkspaceState, EntryContent, EntryNavigation, EntryQuery,
    EntryRecord, EntrySummary, Feed, FeedSummary, UserSettings,
};

/// A page of entries and, when asked for, a count of entries matching a second query.
#[derive(Debug, Clone, PartialEq)]
pub struct EntriesPage {
    pub entries: Vec<EntrySummary>,
    pub counted: Option<u64>,
}

/// Everything the reader shows for one entry.
#[derive(Debug, Clone, PartialEq)]
pub struct ReaderSnapshot {
    pub record: EntryRecord,
    pub content: Option<EntryContent>,
    pub navigation: EntryNavigation,
    /// The entry's subscription while it is active.
    pub feed: Option<Feed>,
}

/// What the entry list starts with.
#[derive(Debug, Clone, PartialEq)]
pub struct EntriesStart {
    pub settings: Option<UserSettings>,
    pub workspace: Option<EntriesWorkspaceState>,
    pub feeds: Option<Vec<FeedSummary>>,
}

/// What the startup target depends on.
#[derive(Debug, Clone, PartialEq)]
pub struct StartupState {
    pub settings: UserSettings,
    /// The last opened subscription, only while it is active.
    pub last_opened_feed_id: Option<i64>,
}

/// The subscriptions page: every active subscription and the fingerprint a configuration
/// import from this page expects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedsOverview {
    pub archived_count: u64,
    pub feeds: Vec<FeedSummary>,
    pub fingerprint: ConfigFingerprint,
}

/// The configuration as exported: the active subscriptions and the settings.
#[derive(Debug, Clone, PartialEq)]
pub struct ConfigSnapshot {
    pub feeds: Vec<Feed>,
    pub settings: UserSettings,
}

#[async_trait::async_trait]
pub trait LibraryQueryPort: Send + Sync {
    /// The entries matching `query` and, when `count` is given, the number matching `count`.
    async fn entries_page(
        &self,
        query: &EntryQuery,
        count: Option<&EntryQuery>,
    ) -> rssr_domain::Result<EntriesPage>;
    /// The entry, its body, its neighbours and its subscription; `None` when it does not exist.
    async fn reader_snapshot(&self, entry_id: i64) -> rssr_domain::Result<Option<ReaderSnapshot>>;
    /// The settings and list preferences (`preferences`) and the subscriptions (`feeds`).
    async fn entries_start(
        &self,
        preferences: bool,
        feeds: bool,
    ) -> rssr_domain::Result<EntriesStart>;
    async fn startup_state(&self) -> rssr_domain::Result<StartupState>;
    async fn feeds_overview(&self) -> rssr_domain::Result<FeedsOverview>;
    async fn config_snapshot(&self) -> rssr_domain::Result<ConfigSnapshot>;
    async fn config_fingerprint(&self) -> rssr_domain::Result<ConfigFingerprint>;
}

/// Answers the queries one repository read at a time — no snapshot. Only for unit tests of the
/// services, which check the use-case logic over stub repositories.
#[cfg(test)]
pub(crate) mod test_support {
    use std::sync::Arc;

    use rssr_domain::{
        AppStateRepository, EntryContentRepository, EntryIndexRepository, EntryQuery,
        FeedRepository, FingerprintFeed, SettingsRepository,
    };

    use super::*;

    #[derive(Clone, Default)]
    pub(crate) struct RepositoryQueries {
        pub(crate) feeds: Option<Arc<dyn FeedRepository>>,
        pub(crate) entries: Option<Arc<dyn EntryIndexRepository>>,
        pub(crate) contents: Option<Arc<dyn EntryContentRepository>>,
        pub(crate) settings: Option<Arc<dyn SettingsRepository>>,
        pub(crate) app_state: Option<Arc<dyn AppStateRepository>>,
    }

    fn needed<T: ?Sized>(port: &Option<Arc<T>>) -> &Arc<T> {
        port.as_ref().expect("this test does not provide the repository the query needs")
    }

    #[async_trait::async_trait]
    impl LibraryQueryPort for RepositoryQueries {
        async fn entries_page(
            &self,
            query: &EntryQuery,
            count: Option<&EntryQuery>,
        ) -> rssr_domain::Result<EntriesPage> {
            let entries = needed(&self.entries).list_entries(query).await?;
            let counted = match count {
                Some(count) => Some(needed(&self.entries).count_entries(count).await?),
                None => None,
            };
            Ok(EntriesPage { entries, counted })
        }

        async fn reader_snapshot(
            &self,
            entry_id: i64,
        ) -> rssr_domain::Result<Option<ReaderSnapshot>> {
            let Some(record) = needed(&self.entries).get_entry_record(entry_id).await? else {
                return Ok(None);
            };
            let content = needed(&self.contents).get_content(entry_id).await?;
            let navigation = needed(&self.entries).reader_navigation(entry_id).await?;
            let feed = needed(&self.feeds).get_feed(record.feed_id).await?;
            Ok(Some(ReaderSnapshot { record, content, navigation, feed }))
        }

        async fn entries_start(
            &self,
            preferences: bool,
            feeds: bool,
        ) -> rssr_domain::Result<EntriesStart> {
            Ok(EntriesStart {
                settings: if preferences {
                    Some(needed(&self.settings).load().await?)
                } else {
                    None
                },
                workspace: if preferences {
                    Some(needed(&self.app_state).load().await?.entries_workspace)
                } else {
                    None
                },
                feeds: if feeds { Some(needed(&self.feeds).list_summaries().await?) } else { None },
            })
        }

        async fn startup_state(&self) -> rssr_domain::Result<StartupState> {
            let settings = needed(&self.settings).load().await?;
            let last_opened = needed(&self.app_state).load().await?.last_opened_feed_id;
            let last_opened_feed_id = match last_opened {
                Some(feed_id) => needed(&self.feeds).get_feed(feed_id).await?.map(|feed| feed.id),
                None => None,
            };
            Ok(StartupState { settings, last_opened_feed_id })
        }

        async fn feeds_overview(&self) -> rssr_domain::Result<FeedsOverview> {
            Ok(FeedsOverview {
                archived_count: 0,
                feeds: needed(&self.feeds).list_summaries().await?,
                fingerprint: self.config_fingerprint().await?,
            })
        }

        /// Repositories hold no generations or raw names: the display name stands in for the
        /// custom one, generation 0 for all.
        async fn config_fingerprint(&self) -> rssr_domain::Result<ConfigFingerprint> {
            let ConfigSnapshot { feeds, settings } = self.config_snapshot().await?;
            let projection = feeds
                .iter()
                .map(|feed| FingerprintFeed {
                    id: feed.id,
                    url: feed.url.as_str(),
                    generation: 0,
                    custom_title: feed.title.as_deref(),
                    folder: feed.folder.as_deref(),
                })
                .collect();
            Ok(ConfigFingerprint::compute(projection, &settings))
        }

        async fn config_snapshot(&self) -> rssr_domain::Result<ConfigSnapshot> {
            let feeds = needed(&self.feeds)
                .list_feeds()
                .await?
                .into_iter()
                .filter(|feed| !feed.is_deleted)
                .collect();
            Ok(ConfigSnapshot { feeds, settings: needed(&self.settings).load().await? })
        }
    }
}
