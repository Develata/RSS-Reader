#![cfg(target_arch = "wasm32")]

use std::sync::{Arc, Mutex};

use rssr_application::import_export_service::{ImportExportService, RemoteConfigStore};
use rssr_domain::{ConfigFeed, ConfigPackage, UserSettings};
use rssr_infra::application_adapters::browser::{
    adapters::{
        BrowserAppStateAdapter, BrowserEntryRepository, BrowserFeedRepository, BrowserOpmlCodec,
        BrowserSettingsRepository,
    },
    state::{LoadedState, PersistedEntry, PersistedFeed, PersistedState, STORAGE_KEY, load_state},
};
use time::OffsetDateTime;
use wasm_bindgen_test::wasm_bindgen_test;

wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

#[derive(Default)]
struct MemoryRemoteConfigStore {
    payload: Mutex<Option<String>>,
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl RemoteConfigStore for MemoryRemoteConfigStore {
    async fn upload_config(&self, raw: &str) -> anyhow::Result<()> {
        *self.payload.lock().expect("lock payload") = Some(raw.to_string());
        Ok(())
    }

    async fn download_config(&self) -> anyhow::Result<Option<String>> {
        Ok(self.payload.lock().expect("lock payload").clone())
    }
}

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

fn sample_entry(id: i64, feed_id: i64, index: i64) -> PersistedEntry {
    PersistedEntry {
        id,
        feed_id,
        external_id: format!("entry-{index}"),
        dedup_key: format!("entry-{index}"),
        url: Some(format!("https://example.com/articles/{index}")),
        title: format!("Entry {index}"),
        author: Some("RSSR".to_string()),
        summary: Some(format!("Summary {index}")),
        content_html: Some(format!("<p>Summary {index}</p>")),
        content_text: Some(format!("Summary {index}")),
        published_at: Some(OffsetDateTime::UNIX_EPOCH),
        updated_at_source: None,
        first_seen_at: OffsetDateTime::UNIX_EPOCH,
        content_hash: Some(format!("hash-{index}")),
        is_read: false,
        is_starred: false,
        read_at: None,
        starred_at: None,
        created_at: OffsetDateTime::UNIX_EPOCH,
        updated_at: OffsetDateTime::UNIX_EPOCH,
    }
}

fn build_service(state: Arc<Mutex<PersistedState>>) -> ImportExportService {
    ImportExportService::new_with_feed_removal_cleanup(
        Arc::new(BrowserFeedRepository::new(state.clone())),
        Arc::new(BrowserEntryRepository::new(state.clone())),
        Arc::new(BrowserSettingsRepository::new(state.clone())),
        Arc::new(BrowserOpmlCodec),
        Arc::new(BrowserAppStateAdapter::new(state)),
    )
}

#[wasm_bindgen_test]
async fn browser_config_exchange_export_json_captures_active_feeds_and_settings() {
    clear_browser_state_storage();

    let settings = UserSettings {
        refresh_interval_minutes: 15,
        custom_css: "[data-page=\"feeds\"] { gap: 8px; }".to_string(),
        ..UserSettings::default()
    };
    let state = Arc::new(Mutex::new(PersistedState {
        next_feed_id: 2,
        feeds: vec![
            sample_feed(1, "https://example.com/feed.xml", false),
            sample_feed(2, "https://example.com/deleted.xml", true),
        ],
        settings: settings.clone(),
        ..PersistedState::default()
    }));
    let service = build_service(state);

    let raw = service.export_config_json().await.expect("export config json");
    let package: ConfigPackage = serde_json::from_str(&raw).expect("decode export json");

    assert_eq!(package.feeds.len(), 1);
    assert_eq!(package.feeds[0].url, "https://example.com/feed.xml");
    assert_eq!(package.settings, settings);

    clear_browser_state_storage();
}

#[wasm_bindgen_test]
async fn browser_config_exchange_import_cleans_removed_feed_entries_and_last_opened_state() {
    clear_browser_state_storage();

    let state = Arc::new(Mutex::new(PersistedState {
        next_feed_id: 2,
        next_entry_id: 1,
        feeds: vec![
            sample_feed(1, "https://example.com/feed.xml", false),
            sample_feed(2, "https://stale.example.com/rss", false),
        ],
        entries: vec![sample_entry(1, 2, 1)],
        last_opened_feed_id: Some(2),
        ..PersistedState::default()
    }));
    let service = build_service(state.clone());

    service
        .import_config_package(&ConfigPackage {
            version: 1,
            exported_at: OffsetDateTime::UNIX_EPOCH,
            feeds: vec![ConfigFeed {
                url: "https://example.com/feed.xml".to_string(),
                title: None,
                folder: None,
            }],
            settings: UserSettings::default(),
        })
        .await
        .expect("import config package");

    {
        let snapshot = state.lock().expect("lock state");
        assert_eq!(snapshot.feeds.len(), 2);
        assert!(snapshot.feeds.iter().find(|feed| feed.id == 2).expect("dropped feed").is_deleted);
        assert!(snapshot.entries.is_empty());
        assert_eq!(snapshot.last_opened_feed_id, None);
    }

    let LoadedState { state: persisted, warning } = load_state();
    assert!(warning.is_none());
    assert!(persisted.feeds.iter().find(|feed| feed.id == 2).expect("dropped feed").is_deleted);
    assert!(persisted.entries.is_empty());
    assert_eq!(persisted.last_opened_feed_id, None);

    clear_browser_state_storage();
}

#[wasm_bindgen_test]
async fn browser_config_exchange_remote_pull_roundtrip_restores_feed_and_settings() {
    clear_browser_state_storage();

    let export_state = Arc::new(Mutex::new(PersistedState {
        next_feed_id: 1,
        feeds: vec![sample_feed(1, "https://example.com/feed.xml", false)],
        settings: UserSettings {
            refresh_interval_minutes: 10,
            custom_css: ".reader-shell { max-width: 72ch; }".to_string(),
            ..UserSettings::default()
        },
        ..PersistedState::default()
    }));
    let export_service = build_service(export_state);
    let remote = Arc::new(MemoryRemoteConfigStore::default());
    export_service.push_remote_config(remote.as_ref()).await.expect("push remote config");

    let import_state = Arc::new(Mutex::new(PersistedState::default()));
    let import_service = build_service(import_state.clone());
    let pulled = import_service.pull_remote_config(remote.as_ref()).await.expect("pull remote");
    assert!(pulled);

    {
        let snapshot = import_state.lock().expect("lock state");
        assert_eq!(snapshot.feeds.len(), 1);
        assert_eq!(snapshot.feeds[0].url, "https://example.com/feed.xml");
        assert_eq!(snapshot.settings.refresh_interval_minutes, 10);
        assert_eq!(snapshot.settings.custom_css, ".reader-shell { max-width: 72ch; }");
    }

    let LoadedState { state: persisted, warning } = load_state();
    assert!(warning.is_none());
    assert_eq!(persisted.feeds.len(), 1);
    assert_eq!(persisted.settings.refresh_interval_minutes, 10);

    clear_browser_state_storage();
}
