#![cfg(target_arch = "wasm32")]

use std::sync::{Arc, Mutex};

use rssr_application::import_export_service::{ClockPort, ImportExportService, RemoteConfigStore};
use rssr_domain::{ConfigFeed, ConfigPackage, UserSettings};
use rssr_infra::application_adapters::browser::{
    adapters::{
        BrowserAppStateAdapter, BrowserEntryRepository, BrowserFeedRepository, BrowserOpmlCodec,
        BrowserSettingsRepository,
    },
    state::{
        APP_STATE_STORAGE_KEY, BrowserState, ENTRY_FLAGS_STORAGE_KEY, LoadedState,
        PersistedAppStateSlice, PersistedEntry, PersistedFeed, PersistedState, STORAGE_KEY,
        load_state,
    },
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

struct FixedClock;

impl ClockPort for FixedClock {
    fn now_utc(&self) -> OffsetDateTime {
        OffsetDateTime::UNIX_EPOCH
    }
}

fn clear_browser_state_storage() {
    if let Some(storage) =
        web_sys::window().and_then(|window| window.local_storage().ok()).flatten()
    {
        let _ = storage.remove_item(STORAGE_KEY);
        let _ = storage.remove_item(APP_STATE_STORAGE_KEY);
        let _ = storage.remove_item(ENTRY_FLAGS_STORAGE_KEY);
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
        created_at: OffsetDateTime::UNIX_EPOCH,
        updated_at: OffsetDateTime::UNIX_EPOCH,
    }
}

fn build_service(state: Arc<Mutex<BrowserState>>) -> ImportExportService {
    ImportExportService::new_with_app_state_cleanup_and_clock(
        Arc::new(BrowserFeedRepository::new(state.clone())),
        Arc::new(BrowserEntryRepository::new(state.clone())),
        Arc::new(BrowserSettingsRepository::new(state.clone())),
        Arc::new(BrowserOpmlCodec),
        Arc::new(BrowserAppStateAdapter::new(state)),
        Arc::new(FixedClock),
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
    let state = Arc::new(Mutex::new(BrowserState {
        core: PersistedState {
            next_feed_id: 2,
            feeds: vec![
                sample_feed(1, "https://example.com/feed.xml", false),
                sample_feed(2, "https://example.com/deleted.xml", true),
            ],
            settings: settings.clone(),
            ..PersistedState::default()
        },
        ..BrowserState::default()
    }));
    let service = build_service(state);

    let raw = service.export_config_json().await.expect("export config json");
    let package: ConfigPackage = serde_json::from_str(&raw).expect("decode export json");

    assert_eq!(package.feeds.len(), 1);
    assert_eq!(package.feeds[0].url, "https://example.com/feed.xml");
    assert_eq!(package.settings, settings);
    assert_eq!(package.exported_at, OffsetDateTime::UNIX_EPOCH);

    clear_browser_state_storage();
}

#[wasm_bindgen_test]
async fn browser_config_exchange_import_cleans_removed_feed_entries_and_last_opened_state() {
    clear_browser_state_storage();

    let state = Arc::new(Mutex::new(BrowserState {
        core: PersistedState {
            next_feed_id: 2,
            next_entry_id: 1,
            feeds: vec![
                sample_feed(1, "https://example.com/feed.xml", false),
                sample_feed(2, "https://stale.example.com/rss", false),
            ],
            entries: vec![sample_entry(1, 2, 1)],
            ..PersistedState::default()
        },
        app_state: PersistedAppStateSlice {
            last_opened_feed_id: Some(2),
            ..PersistedAppStateSlice::default()
        },
        ..BrowserState::default()
    }));
    let service = build_service(state.clone());

    service
        .import_config_package(&ConfigPackage {
            version: 2,
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
        assert_eq!(snapshot.core.feeds.len(), 2);
        assert!(
            snapshot.core.feeds.iter().find(|feed| feed.id == 2).expect("dropped feed").is_deleted
        );
        assert!(snapshot.core.entries.is_empty());
        assert_eq!(snapshot.app_state.last_opened_feed_id, None);
    }

    let LoadedState { state: persisted, warning } = load_state();
    assert!(warning.is_none());
    assert!(
        persisted.core.feeds.iter().find(|feed| feed.id == 2).expect("dropped feed").is_deleted
    );
    assert!(persisted.core.entries.is_empty());
    assert_eq!(persisted.app_state.last_opened_feed_id, None);

    clear_browser_state_storage();
}

#[wasm_bindgen_test]
async fn browser_config_exchange_remote_pull_roundtrip_restores_feed_and_settings() {
    clear_browser_state_storage();

    let export_state = Arc::new(Mutex::new(BrowserState {
        core: PersistedState {
            next_feed_id: 1,
            feeds: vec![sample_feed(1, "https://example.com/feed.xml", false)],
            settings: UserSettings {
                refresh_interval_minutes: 10,
                custom_css: ".reader-shell { max-width: 72ch; }".to_string(),
                ..UserSettings::default()
            },
            ..PersistedState::default()
        },
        ..BrowserState::default()
    }));
    let export_service = build_service(export_state);
    let remote = Arc::new(MemoryRemoteConfigStore::default());
    export_service.push_remote_config(remote.as_ref()).await.expect("push remote config");

    let import_state = Arc::new(Mutex::new(BrowserState::default()));
    let import_service = build_service(import_state.clone());
    let pulled = import_service.pull_remote_config(remote.as_ref()).await.expect("pull remote");
    assert!(pulled.found());
    assert_eq!(pulled.import.as_ref().expect("import outcome").imported_feed_count, 1);

    {
        let snapshot = import_state.lock().expect("lock state");
        assert_eq!(snapshot.core.feeds.len(), 1);
        assert_eq!(snapshot.core.feeds[0].url, "https://example.com/feed.xml");
        assert_eq!(snapshot.core.settings.refresh_interval_minutes, 10);
        assert_eq!(snapshot.core.settings.custom_css, ".reader-shell { max-width: 72ch; }");
    }

    let LoadedState { state: persisted, warning } = load_state();
    assert!(warning.is_none());
    assert_eq!(persisted.core.feeds.len(), 1);
    assert_eq!(persisted.core.settings.refresh_interval_minutes, 10);

    clear_browser_state_storage();
}
