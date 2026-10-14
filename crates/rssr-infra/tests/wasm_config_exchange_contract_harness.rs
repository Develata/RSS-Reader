#![cfg(target_arch = "wasm32")]

use std::sync::{Arc, Mutex};

use rssr_application::import_export_service::{ClockPort, ImportExportService, RemoteConfigStore};
use rssr_application::{SettingsPatch, SettingsUpdatePort};
use rssr_domain::{ConfigFeed, ConfigPackage, UserSettings};
use rssr_infra::application_adapters::browser::{
    adapters::{
        BrowserFeedRepository, BrowserLibraryQueries, BrowserOpmlCodec,
        BrowserPersistenceMutations, BrowserSettingsRepository,
    },
    state::{
        BrowserState, BrowserStore, PersistedAppStateSlice, PersistedEntryContent,
        PersistedEntryContentSlice, PersistedEntryFlag, PersistedEntryFlagsSlice,
        PersistedEntryIndex, PersistedFeed, PersistedState,
    },
};
use time::OffsetDateTime;
use wasm_bindgen_test::wasm_bindgen_test;

wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

#[path = "support/browser_storage.rs"]
#[allow(dead_code)]
mod browser_storage;
use browser_storage::{clear_browser_state_storage, committed_slice, persisted_state, seed_state};

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

fn sample_feed(id: i64, url: &str, is_deleted: bool) -> PersistedFeed {
    PersistedFeed {
        id,
        url: url.to_string(),
        title: Some(format!("Feed {id}")),
        custom_title: None,
        site_url: None,
        description: None,
        icon_url: None,
        folder: None,
        etag: None,
        last_modified: None,
        last_fetched_at: None,
        last_success_at: None,
        fetch_error: None,
        fetch_error_kind: None,
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
        published_at: Some(OffsetDateTime::UNIX_EPOCH),
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

fn build_service(state: BrowserStore) -> ImportExportService {
    ImportExportService::new(
        Arc::new(BrowserFeedRepository::new(state.clone())),
        Arc::new(BrowserLibraryQueries::new(state.clone())),
        Arc::new(BrowserOpmlCodec),
        Arc::new(FixedClock),
        Arc::new(BrowserPersistenceMutations::new(state)),
    )
}

/// A settings patch is merged under the store's lock: a no-op publishes nothing, an invalid
/// patch leaves the settings as they were.
#[wasm_bindgen_test]
async fn browser_settings_updates_merge_in_one_write() {
    clear_browser_state_storage();
    let state = seed_state(BrowserState::default()).await;
    let settings = BrowserSettingsRepository::new(state);

    let core_before =
        committed_slice(rssr_infra::application_adapters::browser::state::STORAGE_KEY);
    let same = SettingsPatch { theme: Some(UserSettings::default().theme), ..Default::default() };
    assert_eq!(settings.update_settings(same).await.unwrap(), UserSettings::default());
    assert_eq!(
        committed_slice(rssr_infra::application_adapters::browser::state::STORAGE_KEY),
        core_before,
        "a no-op publishes nothing"
    );

    let patch = SettingsPatch { refresh_interval_minutes: Some(45), ..Default::default() };
    let saved = settings.update_settings(patch).await.unwrap();
    assert_eq!(saved.refresh_interval_minutes, 45);
    assert_eq!(persisted_state().await.core.settings, saved);

    let invalid = SettingsPatch { entries_page_size: Some(0), ..Default::default() };
    assert!(settings.update_settings(invalid).await.is_err());
    assert_eq!(persisted_state().await.core.settings, saved);
    clear_browser_state_storage();
}

#[wasm_bindgen_test]
async fn browser_config_exchange_export_json_captures_active_feeds_and_settings() {
    clear_browser_state_storage();

    let settings = UserSettings {
        refresh_interval_minutes: 15,
        custom_css: "[data-page=\"feeds\"] { gap: 8px; }".to_string(),
        ..UserSettings::default()
    };
    let state = seed_state(BrowserState {
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
    })
    .await;
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

    let state = seed_state(BrowserState {
        core: PersistedState {
            next_feed_id: 2,
            next_entry_id: 1,
            feeds: vec![
                sample_feed(1, "https://example.com/feed.xml", false),
                sample_feed(2, "https://stale.example.com/rss", false),
            ],
            entries: vec![sample_entry_index(1, 2, 1)],
            ..PersistedState::default()
        },
        entry_content: PersistedEntryContentSlice { entries: vec![sample_entry_content(1, 2, 1)] },
        entry_flags: PersistedEntryFlagsSlice {
            entries: vec![PersistedEntryFlag {
                id: 1,
                is_read: false,
                is_starred: false,
                read_at: None,
                starred_at: None,
            }],
        },
        app_state: PersistedAppStateSlice {
            last_opened_feed_id: Some(2),
            ..PersistedAppStateSlice::default()
        },
    })
    .await;
    let service = build_service(state.clone());
    let current = service.config_fingerprint().await.unwrap();

    service
        .import_config_package(
            &ConfigPackage {
                version: 2,
                exported_at: OffsetDateTime::UNIX_EPOCH,
                feeds: vec![ConfigFeed {
                    url: "https://example.com/feed.xml".to_string(),
                    title: None,
                    folder: None,
                }],
                settings: UserSettings::default(),
            },
            &current,
        )
        .await
        .expect("import config package");

    {
        let snapshot = state.snapshot().await.expect("snapshot");
        assert_eq!(snapshot.core.feeds.len(), 2);
        assert!(
            snapshot.core.feeds.iter().find(|feed| feed.id == 2).expect("dropped feed").is_deleted
        );
        assert!(snapshot.core.entries.is_empty());
        assert!(snapshot.entry_content.entries.is_empty());
        assert_eq!(snapshot.app_state.last_opened_feed_id, None);
    }

    let persisted = persisted_state().await;
    assert!(
        persisted.core.feeds.iter().find(|feed| feed.id == 2).expect("dropped feed").is_deleted
    );
    assert!(persisted.core.entries.is_empty());
    assert!(persisted.entry_content.entries.is_empty());
    assert_eq!(persisted.app_state.last_opened_feed_id, None);

    clear_browser_state_storage();
}

#[wasm_bindgen_test]
async fn browser_config_exchange_remote_pull_roundtrip_restores_feed_and_settings() {
    clear_browser_state_storage();

    let export_state = seed_state(BrowserState {
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
    })
    .await;
    let export_service = build_service(export_state);
    let remote = Arc::new(MemoryRemoteConfigStore::default());
    export_service.push_remote_config(remote.as_ref()).await.expect("push remote config");

    let import_state = seed_state(BrowserState::default()).await;
    let import_service = build_service(import_state.clone());
    let pulled = import_service.pull_remote_config(remote.as_ref()).await.expect("pull remote");
    assert!(pulled.found());
    assert_eq!(pulled.import.as_ref().expect("import outcome").imported_feed_count, 1);

    {
        let snapshot = import_state.snapshot().await.expect("snapshot");
        assert_eq!(snapshot.core.feeds.len(), 1);
        assert_eq!(snapshot.core.feeds[0].url, "https://example.com/feed.xml");
        assert_eq!(snapshot.core.settings.refresh_interval_minutes, 10);
        assert_eq!(snapshot.core.settings.custom_css, ".reader-shell { max-width: 72ch; }");
    }

    let persisted = persisted_state().await;
    assert_eq!(persisted.core.feeds.len(), 1);
    assert_eq!(persisted.core.settings.refresh_interval_minutes, 10);

    clear_browser_state_storage();
}

/// Web 端 OPML 委托给共用的 `OpmlCodec`（审计 D5）：导入取 `text` 作为指定名称、源标题不动，
/// 导出 `text` 写显示名称、`title` 写源标题（审计 B6）；残缺的文档在写入前整体拒绝（原先
/// Web 端的副本会接受）。
#[wasm_bindgen_test]
async fn browser_opml_uses_the_shared_codec_and_its_name_rules() {
    clear_browser_state_storage();
    let state = seed_state(BrowserState {
        core: PersistedState {
            next_feed_id: 1,
            feeds: vec![sample_feed(1, "https://example.com/feed.xml", false)],
            ..PersistedState::default()
        },
        ..BrowserState::default()
    })
    .await;
    let service = build_service(state);

    service
        .import_opml(
            r#"<opml version="2.0"><body>
                <outline text="工作资讯" title="Example News" type="rss" xmlUrl="https://example.com/feed.xml"/>
            </body></opml>"#,
        )
        .await
        .expect("import opml");
    let persisted = persisted_state().await;
    assert_eq!(persisted.core.feeds[0].custom_title.as_deref(), Some("工作资讯"));
    assert_eq!(persisted.core.feeds[0].title.as_deref(), Some("Feed 1"), "source title untouched");

    let exported = service.export_opml().await.expect("export opml");
    assert!(
        exported.contains(
            r#"text="工作资讯" title="Feed 1" type="rss" xmlUrl="https://example.com/feed.xml""#
        ),
        "{exported}"
    );

    let truncated = service
        .import_opml(
            r#"<opml version="2.0"><body><outline text="New" type="rss" xmlUrl="https://new.example/feed"/>"#,
        )
        .await;
    assert!(truncated.is_err(), "a truncated document is rejected");
    let outside_body = service
        .import_opml(
            r#"<opml version="2.0"><head/><body></body><outline text="New" type="rss" xmlUrl="https://new.example/feed"/></opml>"#,
        )
        .await;
    assert!(outside_body.is_err(), "a feed outside <body> is not silently skipped");
    assert_eq!(persisted_state().await.core.feeds.len(), 1, "nothing was written");
    clear_browser_state_storage();
}
