#![cfg(target_arch = "wasm32")]

use std::sync::{Arc, Mutex};

use rssr_application::{
    FeedRefreshUpdate, ParsedEntryData, ParsedFeedUpdate, RefreshCommit, RefreshFailure,
    RefreshHttpMetadata, RefreshStorePort,
};
use rssr_infra::application_adapters::browser::{
    adapters::BrowserRefreshStore,
    state::{LoadedState, PersistedFeed, PersistedState, STORAGE_KEY, load_state},
};
use time::OffsetDateTime;
use url::Url;
use wasm_bindgen_test::wasm_bindgen_test;

wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

fn clear_browser_state_storage() {
    if let Some(storage) =
        web_sys::window().and_then(|window| window.local_storage().ok()).flatten()
    {
        let _ = storage.remove_item(STORAGE_KEY);
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

fn sample_entry(index: i64) -> ParsedEntryData {
    ParsedEntryData {
        external_id: format!("entry-{index}"),
        dedup_key: format!("entry-{index}"),
        url: Some(Url::parse(&format!("https://example.com/articles/{index}")).expect("valid url")),
        title: format!("Entry {index}"),
        author: Some("RSSR".to_string()),
        summary: Some(format!("Summary {index}")),
        content_html: Some(format!("<p>Summary {index}</p>")),
        content_text: Some(format!("Summary {index}")),
        published_at: Some(OffsetDateTime::UNIX_EPOCH + time::Duration::days(index)),
        updated_at_source: None,
    }
}

#[wasm_bindgen_test]
async fn browser_refresh_store_lists_only_active_targets() {
    clear_browser_state_storage();

    let state = Arc::new(Mutex::new(PersistedState {
        next_feed_id: 2,
        feeds: vec![
            sample_feed(1, "https://example.com/feed-1.xml", false),
            sample_feed(2, "https://example.com/feed-2.xml", true),
        ],
        ..PersistedState::default()
    }));
    let store = BrowserRefreshStore::new(state);

    let targets = store.list_targets().await.expect("list targets");

    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].feed_id, 1);
    assert_eq!(targets[0].url.as_str(), "https://example.com/feed-1.xml");

    clear_browser_state_storage();
}

#[wasm_bindgen_test]
async fn browser_refresh_store_commit_not_modified_updates_state_and_storage() {
    clear_browser_state_storage();

    let state = Arc::new(Mutex::new(PersistedState {
        next_feed_id: 1,
        feeds: vec![sample_feed(1, "https://example.com/feed.xml", false)],
        ..PersistedState::default()
    }));
    let store = BrowserRefreshStore::new(state.clone());

    store
        .commit(
            1,
            RefreshCommit::NotModified {
                metadata: RefreshHttpMetadata {
                    etag: Some("etag-1".to_string()),
                    last_modified: Some("Wed, 01 Apr 2026 10:00:00 GMT".to_string()),
                },
            },
        )
        .await
        .expect("commit not modified");

    {
        let snapshot = state.lock().expect("lock state");
        assert_eq!(snapshot.feeds.len(), 1);
        assert_eq!(snapshot.feeds[0].etag.as_deref(), Some("etag-1"));
        assert_eq!(
            snapshot.feeds[0].last_modified.as_deref(),
            Some("Wed, 01 Apr 2026 10:00:00 GMT")
        );
        assert!(snapshot.feeds[0].last_fetched_at.is_some());
        assert!(snapshot.feeds[0].last_success_at.is_some());
        assert_eq!(snapshot.feeds[0].fetch_error, None);
    }

    let LoadedState { state: persisted, warning } = load_state();
    assert!(warning.is_none());
    assert_eq!(persisted.feeds.len(), 1);
    assert_eq!(persisted.feeds[0].etag.as_deref(), Some("etag-1"));

    clear_browser_state_storage();
}

#[wasm_bindgen_test]
async fn browser_refresh_store_commit_updated_persists_feed_metadata_and_entries() {
    clear_browser_state_storage();

    let state = Arc::new(Mutex::new(PersistedState {
        next_feed_id: 1,
        feeds: vec![sample_feed(1, "https://example.com/feed.xml", false)],
        ..PersistedState::default()
    }));
    let store = BrowserRefreshStore::new(state.clone());

    store
        .commit(
            1,
            RefreshCommit::Updated {
                update: FeedRefreshUpdate {
                    metadata: RefreshHttpMetadata {
                        etag: Some("etag-updated".to_string()),
                        last_modified: Some("Thu, 02 Apr 2026 10:00:00 GMT".to_string()),
                    },
                    feed: ParsedFeedUpdate {
                        title: Some("Updated Feed".to_string()),
                        site_url: Some(Url::parse("https://example.com").expect("valid site url")),
                        description: Some("Updated description".to_string()),
                        entries: vec![sample_entry(1), sample_entry(2)],
                    },
                },
            },
        )
        .await
        .expect("commit updated");

    {
        let snapshot = state.lock().expect("lock state");
        assert_eq!(snapshot.feeds.len(), 1);
        assert_eq!(snapshot.feeds[0].title.as_deref(), Some("Updated Feed"));
        assert_eq!(snapshot.feeds[0].site_url.as_deref(), Some("https://example.com/"));
        assert_eq!(snapshot.feeds[0].description.as_deref(), Some("Updated description"));
        assert_eq!(snapshot.feeds[0].etag.as_deref(), Some("etag-updated"));
        assert!(snapshot.feeds[0].last_fetched_at.is_some());
        assert!(snapshot.feeds[0].last_success_at.is_some());
        assert_eq!(snapshot.feeds[0].fetch_error, None);
        assert_eq!(snapshot.entries.len(), 2);
        assert_eq!(snapshot.entries[0].feed_id, 1);
        assert_eq!(snapshot.entries[0].title, "Entry 1");
        assert_eq!(snapshot.entries[1].title, "Entry 2");
    }

    let LoadedState { state: persisted, warning } = load_state();
    assert!(warning.is_none());
    assert_eq!(persisted.entries.len(), 2);
    assert_eq!(persisted.feeds[0].title.as_deref(), Some("Updated Feed"));

    clear_browser_state_storage();
}

#[wasm_bindgen_test]
async fn browser_refresh_store_commit_failed_persists_error_without_success_timestamp() {
    clear_browser_state_storage();

    let state = Arc::new(Mutex::new(PersistedState {
        next_feed_id: 1,
        feeds: vec![sample_feed(1, "https://example.com/feed.xml", false)],
        ..PersistedState::default()
    }));
    let store = BrowserRefreshStore::new(state.clone());

    store
        .commit(
            1,
            RefreshCommit::Failed {
                failure: RefreshFailure {
                    message: "network timeout".to_string(),
                    metadata: Some(RefreshHttpMetadata {
                        etag: Some("etag-failed".to_string()),
                        last_modified: Some("Fri, 03 Apr 2026 10:00:00 GMT".to_string()),
                    }),
                },
            },
        )
        .await
        .expect("commit failed");

    {
        let snapshot = state.lock().expect("lock state");
        assert_eq!(snapshot.feeds[0].etag.as_deref(), Some("etag-failed"));
        assert_eq!(
            snapshot.feeds[0].last_modified.as_deref(),
            Some("Fri, 03 Apr 2026 10:00:00 GMT")
        );
        assert!(snapshot.feeds[0].last_fetched_at.is_some());
        assert!(snapshot.feeds[0].last_success_at.is_none());
        assert_eq!(snapshot.feeds[0].fetch_error.as_deref(), Some("network timeout"));
        assert!(snapshot.entries.is_empty());
    }

    let LoadedState { state: persisted, warning } = load_state();
    assert!(warning.is_none());
    assert_eq!(persisted.feeds[0].fetch_error.as_deref(), Some("network timeout"));

    clear_browser_state_storage();
}
