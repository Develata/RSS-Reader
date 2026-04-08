#![cfg(target_arch = "wasm32")]

use std::sync::{Arc, Mutex};

use rssr_application::{RefreshCommit, RefreshHttpMetadata, RefreshStorePort};
use rssr_infra::application_adapters::browser::{
    adapters::BrowserRefreshStore,
    state::{LoadedState, PersistedFeed, PersistedState, STORAGE_KEY, load_state},
};
use time::OffsetDateTime;
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
