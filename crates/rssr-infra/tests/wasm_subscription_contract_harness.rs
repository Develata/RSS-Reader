#![cfg(target_arch = "wasm32")]

use std::sync::{Arc, Mutex};

use anyhow::{Result, bail};
use rssr_application::{
    AddSubscriptionInput, FeedRefreshSourceOutput, FeedRefreshSourcePort, FeedService,
    RefreshCommit, RefreshService, RefreshStorePort, RemoveSubscriptionInput, SubscriptionWorkflow,
};
use rssr_domain::{EntryIndexRepository, EntryQuery};
use rssr_infra::application_adapters::browser::{
    adapters::{BrowserAppStateAdapter, BrowserEntryRepository, BrowserFeedRepository},
    state::{
        APP_STATE_STORAGE_KEY, BrowserState, ENTRY_CONTENT_STORAGE_KEY, ENTRY_FLAGS_STORAGE_KEY,
        LoadedState, PersistedAppStateSlice, PersistedEntryContent, PersistedEntryContentSlice,
        PersistedEntryIndex, PersistedFeed, PersistedState, STORAGE_KEY, load_state,
    },
};
use time::OffsetDateTime;
use url::Url;
use wasm_bindgen_test::wasm_bindgen_test;

wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

struct UnusedRefreshSource;
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl rssr_application::SubscriptionProbePort for UnusedRefreshSource {
    async fn probe(&self, url: &Url) -> Result<rssr_application::SubscriptionProbeOutcome> {
        Ok(rssr_application::SubscriptionProbeOutcome::Feed {
            url: url.clone(),
            update: rssr_application::FeedRefreshUpdate {
                metadata: Default::default(),
                feed: rssr_application::ParsedFeedUpdate {
                    title: None,
                    site_url: Some(Url::parse("https://site.example/").unwrap()),
                    description: None,
                    entries: Vec::new(),
                },
            },
        })
    }
}

#[async_trait::async_trait(?Send)]
impl FeedRefreshSourcePort for UnusedRefreshSource {
    async fn refresh(
        &self,
        _target: &rssr_application::RefreshTarget,
    ) -> Result<FeedRefreshSourceOutput> {
        bail!("refresh source should not be used in subscription harness")
    }
}

struct UnusedRefreshStore;

#[async_trait::async_trait]
impl RefreshStorePort for UnusedRefreshStore {
    async fn list_targets(&self) -> Result<Vec<rssr_application::RefreshTarget>> {
        bail!("refresh store should not be used in subscription harness")
    }

    async fn get_target(&self, _feed_id: i64) -> Result<Option<rssr_application::RefreshTarget>> {
        bail!("refresh store should not be used in subscription harness")
    }

    async fn commit(
        &self,
        _feed_id: i64,
        _commit: RefreshCommit,
    ) -> Result<rssr_application::RefreshCommitOutcome> {
        bail!("refresh store should not be used in subscription harness")
    }
}

fn clear_browser_state_storage() {
    if let Some(storage) =
        web_sys::window().and_then(|window| window.local_storage().ok()).flatten()
    {
        let _ = storage.remove_item(STORAGE_KEY);
        let _ = storage.remove_item(APP_STATE_STORAGE_KEY);
        let _ = storage.remove_item(ENTRY_FLAGS_STORAGE_KEY);
        let _ = storage.remove_item(ENTRY_CONTENT_STORAGE_KEY);
    }
}

fn sample_feed(id: i64, url: &str, is_deleted: bool) -> PersistedFeed {
    PersistedFeed {
        id,
        url: url.to_string(),
        title: Some(format!("Feed {id}")),
        site_url: None,
        description: None,
        icon_url: None,
        folder: None,
        etag: None,
        last_modified: None,
        last_fetched_at: None,
        last_success_at: None,
        fetch_error: None,
        is_deleted,
        created_at: OffsetDateTime::UNIX_EPOCH,
        updated_at: OffsetDateTime::UNIX_EPOCH,
    }
}

fn sample_entry_index(id: i64, feed_id: i64, index: i64) -> PersistedEntryIndex {
    PersistedEntryIndex {
        id,
        feed_id,
        external_id: format!("entry-{index}"),
        dedup_key: format!("entry-{index}"),
        url: Some(format!("https://example.com/articles/{index}")),
        title: format!("Entry {index}"),
        author: Some("RSSR".to_string()),
        summary: Some(format!("Summary {index}")),
        published_at: Some(OffsetDateTime::UNIX_EPOCH + time::Duration::days(index)),
        updated_at_source: None,
        first_seen_at: OffsetDateTime::UNIX_EPOCH,
        has_content: true,
        created_at: OffsetDateTime::UNIX_EPOCH,
        updated_at: OffsetDateTime::UNIX_EPOCH,
    }
}

fn sample_entry_content(id: i64, feed_id: i64, index: i64) -> PersistedEntryContent {
    PersistedEntryContent {
        entry_id: id,
        feed_id,
        content_html: Some(format!("<p>Summary {index}</p>")),
        content_text: Some(format!("Summary {index}")),
        content_hash: Some(format!("hash-{index}")),
        updated_at: OffsetDateTime::UNIX_EPOCH,
    }
}

fn build_workflow(state: Arc<Mutex<BrowserState>>) -> SubscriptionWorkflow {
    let entry_repository = Arc::new(BrowserEntryRepository::new(state.clone()));
    let feed_service = FeedService::new(
        Arc::new(BrowserFeedRepository::new(state.clone())),
        entry_repository.clone(),
        entry_repository,
    );
    let refresh_service =
        RefreshService::new(Arc::new(UnusedRefreshSource), Arc::new(UnusedRefreshStore));
    let app_state = Arc::new(BrowserAppStateAdapter::new(state));
    SubscriptionWorkflow::new(
        feed_service,
        refresh_service,
        app_state,
        Arc::new(UnusedRefreshSource),
    )
}

#[wasm_bindgen_test]
async fn browser_subscription_add_normalizes_and_deduplicates_urls() {
    clear_browser_state_storage();

    let state = Arc::new(Mutex::new(BrowserState::default()));
    let workflow = build_workflow(state.clone());

    let first = workflow
        .add_subscription(&AddSubscriptionInput {
            url: "https://example.com:443/feed.xml#fragment".to_string(),
            title: Some("Example".to_string()),
            folder: Some("Inbox".to_string()),
        })
        .await
        .expect("add first subscription");
    let second = workflow
        .add_subscription(&AddSubscriptionInput {
            url: "https://example.com/feed.xml".to_string(),
            title: Some("Updated Title".to_string()),
            folder: Some("Reading".to_string()),
        })
        .await
        .expect_err("duplicate must not modify existing subscription");

    assert!(second.to_string().contains("已订阅"));
    assert_eq!(first.site_url.as_ref().unwrap().as_str(), "https://site.example/");
    assert_eq!(first.url.as_str(), "https://example.com/feed.xml");

    {
        let snapshot = state.lock().expect("lock state");
        assert_eq!(snapshot.core.feeds.len(), 1);
        assert_eq!(snapshot.core.feeds[0].url, "https://example.com/feed.xml");
        assert_eq!(snapshot.core.feeds[0].title.as_deref(), Some("Example"));
        assert_eq!(snapshot.core.feeds[0].folder.as_deref(), Some("Inbox"));
        assert!(!snapshot.core.feeds[0].is_deleted);
    }

    let LoadedState { state: persisted, warning } = load_state();
    assert!(warning.is_none());
    assert_eq!(persisted.core.feeds.len(), 1);
    assert_eq!(persisted.core.feeds[0].url, "https://example.com/feed.xml");
    assert_eq!(persisted.core.feeds[0].title.as_deref(), Some("Example"));

    clear_browser_state_storage();
}

#[wasm_bindgen_test]
async fn browser_subscription_remove_purges_entries_soft_deletes_feed_and_clears_matching_state() {
    clear_browser_state_storage();

    let state = Arc::new(Mutex::new(BrowserState {
        core: PersistedState {
            next_feed_id: 1,
            next_entry_id: 2,
            feeds: vec![sample_feed(1, "https://example.com/feed.xml", false)],
            entries: vec![sample_entry_index(1, 1, 1), sample_entry_index(2, 1, 2)],
            ..PersistedState::default()
        },
        entry_content: PersistedEntryContentSlice {
            entries: vec![sample_entry_content(1, 1, 1), sample_entry_content(2, 1, 2)],
        },
        app_state: PersistedAppStateSlice {
            last_opened_feed_id: Some(1),
            ..PersistedAppStateSlice::default()
        },
        ..BrowserState::default()
    }));
    let workflow = build_workflow(state.clone());

    workflow
        .remove_subscription(RemoveSubscriptionInput { feed_id: 1, purge_entries: true })
        .await
        .expect("remove subscription");

    {
        let snapshot = state.lock().expect("lock state");
        assert_eq!(snapshot.core.feeds.len(), 1);
        assert!(snapshot.core.feeds[0].is_deleted);
        assert!(snapshot.core.entries.is_empty());
        assert!(snapshot.entry_content.entries.is_empty());
        assert_eq!(snapshot.app_state.last_opened_feed_id, None);
    }

    let LoadedState { state: persisted, warning } = load_state();
    assert!(warning.is_none());
    assert_eq!(persisted.core.feeds.len(), 1);
    assert!(persisted.core.feeds[0].is_deleted);
    assert!(persisted.core.entries.is_empty());
    assert!(persisted.entry_content.entries.is_empty());
    assert_eq!(persisted.app_state.last_opened_feed_id, None);

    clear_browser_state_storage();
}

#[wasm_bindgen_test]
async fn browser_subscription_remove_preserves_other_last_opened_feed() {
    clear_browser_state_storage();

    let state = Arc::new(Mutex::new(BrowserState {
        core: PersistedState {
            next_feed_id: 2,
            feeds: vec![
                sample_feed(1, "https://example.com/retained.xml", false),
                sample_feed(2, "https://example.com/removed.xml", false),
            ],
            ..PersistedState::default()
        },
        app_state: PersistedAppStateSlice {
            last_opened_feed_id: Some(1),
            ..PersistedAppStateSlice::default()
        },
        ..BrowserState::default()
    }));
    let workflow = build_workflow(state.clone());

    workflow
        .remove_subscription(RemoveSubscriptionInput { feed_id: 2, purge_entries: false })
        .await
        .expect("remove subscription");

    {
        let snapshot = state.lock().expect("lock state");
        assert_eq!(snapshot.app_state.last_opened_feed_id, Some(1));
        assert!(
            snapshot.core.feeds.iter().find(|feed| feed.id == 2).expect("removed feed").is_deleted
        );
    }

    let entry_repository = BrowserEntryRepository::new(state.clone());
    let entries = entry_repository
        .list_entries(&EntryQuery { feed_id: Some(2), ..EntryQuery::default() })
        .await
        .expect("list entries");
    assert!(entries.is_empty());

    let LoadedState { state: persisted, warning } = load_state();
    assert!(warning.is_none());
    assert_eq!(persisted.app_state.last_opened_feed_id, Some(1));

    clear_browser_state_storage();
}

#[wasm_bindgen_test]
async fn reader_metadata_and_global_unread_counts_survive_failed_flag_writes() {
    use rssr_application::ReaderService;
    use rssr_domain::FeedRepository;
    clear_browser_state_storage();
    let state = Arc::new(Mutex::new(BrowserState {
        core: PersistedState {
            feeds: vec![sample_feed(1, "https://example.com/feed.xml", false)],
            entries: vec![sample_entry_index(1, 1, 1), sample_entry_index(2, 1, 2)],
            ..Default::default()
        },
        entry_content: PersistedEntryContentSlice { entries: vec![sample_entry_content(1, 1, 1)] },
        ..Default::default()
    }));
    let entries = Arc::new(BrowserEntryRepository::new(state.clone()));
    let feeds = Arc::new(BrowserFeedRepository::new(state.clone()));
    let reader = ReaderService::new(entries.clone(), entries.clone(), feeds.clone());
    let snapshot = reader.load_entry(1).await.unwrap();
    assert_eq!(snapshot.feed_title, feeds.get_feed(1).await.unwrap().unwrap().title);
    assert!(snapshot.entry.unwrap().content_html.is_some());
    assert_eq!(feeds.list_summaries().await.unwrap()[0].unread_count, 2);
    entries.set_read(1, true).await.unwrap();
    assert_eq!(feeds.list_summaries().await.unwrap()[0].unread_count, 1);
    entries.set_read(1, true).await.unwrap();
    assert_eq!(feeds.list_summaries().await.unwrap()[0].unread_count, 1);
    entries
        .list_entries(&EntryQuery {
            search_title: Some("does not match".into()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(feeds.list_summaries().await.unwrap()[0].unread_count, 1);
    entries.set_read(2, true).await.unwrap();
    assert_eq!(feeds.list_summaries().await.unwrap()[0].unread_count, 0);
    entries.set_read(1, false).await.unwrap();
    assert_eq!(feeds.list_summaries().await.unwrap()[0].unread_count, 1);

    // Force the browser's real storage API to fail, exercising rollback for an existing flag
    // and a newly inserted flag. Restore the API before asserting to avoid test contamination.
    {
        let mut baseline = state.lock().unwrap();
        baseline.entry_flags.entries.retain(|entry| entry.id != 2);
        rssr_infra::application_adapters::browser::state::save_entry_flags_slice(
            &baseline.entry_flags,
        )
        .unwrap();
    }
    let storage = web_sys::window().unwrap().local_storage().unwrap().unwrap();
    let persisted_before = storage.get_item(ENTRY_FLAGS_STORAGE_KEY).unwrap();
    js_sys::eval("globalThis.__rssrSetItem = Storage.prototype.setItem; Storage.prototype.setItem = function() { throw new DOMException('test quota', 'QuotaExceededError'); };").unwrap();
    let failed = entries.set_read(1, true).await;
    let failed_new = entries.set_read(2, true).await;
    js_sys::eval(
        "Storage.prototype.setItem = globalThis.__rssrSetItem; delete globalThis.__rssrSetItem;",
    )
    .unwrap();
    assert!(failed.is_err());
    assert!(failed_new.is_err());
    assert_eq!(storage.get_item(ENTRY_FLAGS_STORAGE_KEY).unwrap(), persisted_before);
    assert!(!entries.get_entry_record(1).await.unwrap().unwrap().is_read);
    assert!(!entries.get_entry_record(2).await.unwrap().unwrap().is_read);
    assert_eq!(feeds.list_summaries().await.unwrap()[0].unread_count, 2);
    clear_browser_state_storage();
}

#[path = "support/bulk_read_cases.rs"]
mod bulk_cases;

#[wasm_bindgen_test]
async fn browser_bulk_read_matches_sqlite_cases_and_rolls_back_failed_storage() {
    use rssr_domain::MarkReadOutcome;
    use rssr_infra::application_adapters::browser::state::PersistedEntryFlag;
    clear_browser_state_storage();
    let mut browser = BrowserState {
        core: PersistedState {
            feeds: vec![
                sample_feed(1, "https://example.com/1", false),
                sample_feed(2, "https://example.com/2", false),
            ],
            entries: bulk_cases::ROWS
                .iter()
                .map(|&(id, feed_id, title, old, _, _)| {
                    let mut entry = sample_entry_index(id, feed_id, id);
                    entry.title = title.into();
                    entry.published_at = bulk_cases::published(id, old);
                    entry
                })
                .collect(),
            ..Default::default()
        },
        ..Default::default()
    };
    browser.entry_flags.entries = bulk_cases::ROWS
        .iter()
        .map(|&(id, _, _, _, starred, read)| PersistedEntryFlag {
            id,
            is_read: read,
            is_starred: starred,
            read_at: None,
            starred_at: None,
        })
        .collect();
    let state = Arc::new(Mutex::new(browser));
    let entries = BrowserEntryRepository::new(state.clone());
    for (query, expected) in bulk_cases::cases() {
        let preview = entries.preview_mark_read(&query).await.unwrap();
        assert_eq!(preview.unread_entry_ids, expected, "{query:?}");
        assert_eq!(preview.query.limit, None);
    }
    let original = entries.preview_mark_read(&EntryQuery::default()).await.unwrap();
    entries.set_read(1, true).await.unwrap();
    entries.set_read(5, false).await.unwrap();
    assert!(matches!(
        entries.mark_read_if_unchanged(&original).await.unwrap(),
        MarkReadOutcome::SelectionChanged { .. }
    ));
    let fresh = entries.preview_mark_read(&EntryQuery::default()).await.unwrap();
    let before = serde_json::to_value(&state.lock().unwrap().entry_flags).unwrap();
    js_sys::eval("globalThis.__bulkSet = Storage.prototype.setItem; globalThis.__bulkWrites = 0; Storage.prototype.setItem = function(k,v) { if(k==='rssr-web-entry-flags-v1'){globalThis.__bulkWrites++;throw new DOMException('quota','QuotaExceededError');}return globalThis.__bulkSet.call(this,k,v); };").unwrap();
    let failed = entries.mark_read_if_unchanged(&fresh).await;
    let writes = js_sys::eval("globalThis.__bulkWrites").unwrap().as_f64().unwrap();
    js_sys::eval("Storage.prototype.setItem=globalThis.__bulkSet;delete globalThis.__bulkSet;delete globalThis.__bulkWrites;").unwrap();
    assert!(failed.is_err());
    assert_eq!(writes, 1.0);
    assert_eq!(serde_json::to_value(&state.lock().unwrap().entry_flags).unwrap(), before);
    js_sys::eval("globalThis.__bulkSet = Storage.prototype.setItem;globalThis.__bulkWrites=0;Storage.prototype.setItem=function(k,v){if(k==='rssr-web-entry-flags-v1')globalThis.__bulkWrites++;return globalThis.__bulkSet.call(this,k,v);};").unwrap();
    let result = entries.mark_read_if_unchanged(&fresh).await;
    let writes = js_sys::eval("globalThis.__bulkWrites").unwrap().as_f64().unwrap();
    js_sys::eval("Storage.prototype.setItem=globalThis.__bulkSet;delete globalThis.__bulkSet;delete globalThis.__bulkWrites;").unwrap();
    assert_eq!(result.unwrap(), MarkReadOutcome::Applied { changed_count: 5 });
    assert_eq!(writes, 1.0);
    for id in fresh.unread_entry_ids {
        assert!(entries.get_entry_record(id).await.unwrap().unwrap().read_at.is_some());
    }
    let empty = entries.preview_mark_read(&EntryQuery::default()).await.unwrap();
    assert_eq!(
        entries.mark_read_if_unchanged(&empty).await.unwrap(),
        MarkReadOutcome::Applied { changed_count: 0 }
    );
    clear_browser_state_storage();
}

#[path = "support/discovery_cases.rs"]
mod discovery_cases;
#[wasm_bindgen_test]
fn browser_discovery_parser_matches_native() {
    discovery_cases::assert_discovery_cases();
}
