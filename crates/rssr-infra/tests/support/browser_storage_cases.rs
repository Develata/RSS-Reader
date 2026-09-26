//! Repository-level concurrency and failure contracts, exercised in a real browser.
use super::{browser_storage::*, sample_entry_content, sample_entry_index, sample_feed};
use rssr_domain::{EntryIndexRepository, FeedRepository, SettingsRepository};
use rssr_infra::application_adapters::browser::{
    adapters::{BrowserEntryRepository, BrowserFeedRepository, BrowserSettingsRepository},
    state::{BrowserState, BrowserStore, COMMIT_STORAGE_KEY, ENTRY_FLAGS_STORAGE_KEY, STORAGE_KEY},
};
use wasm_bindgen_test::wasm_bindgen_test;

fn fixture() -> BrowserState {
    let mut state = BrowserState::default();
    state.core.next_feed_id = 1;
    state.core.next_entry_id = 2;
    state.core.feeds.push(sample_feed(1, "https://example.com/feed.xml", false));
    state.core.entries = vec![sample_entry_index(1, 1, 1), sample_entry_index(2, 1, 2)];
    state.entry_content.entries.push(sample_entry_content(1, 1, 1));
    state
}

async fn eval_promise(script: &str) {
    wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(js_sys::eval(script).unwrap()))
        .await
        .unwrap();
}

#[wasm_bindgen_test]
async fn stale_stores_preserve_other_entries_and_other_bits_with_concurrent_writes() {
    let first = seed_state(fixture()).await;
    let second = BrowserStore::open().await.unwrap();
    let a = BrowserEntryRepository::new(first);
    let b = BrowserEntryRepository::new(second);
    let (read, star) = tokio::join!(a.set_read(1, true), b.set_starred(1, true));
    read.unwrap();
    star.unwrap();
    let (first_star, second_star) = tokio::join!(a.set_starred(1, false), b.set_starred(2, true));
    first_star.unwrap();
    second_star.unwrap();
    let first = a.get_entry_record(1).await.unwrap().unwrap();
    assert!(first.is_read);
    assert!(!first.is_starred);
    assert!(a.get_entry_record(2).await.unwrap().unwrap().is_starred);
    let reopened = BrowserEntryRepository::new(BrowserStore::open().await.unwrap());
    assert!(reopened.get_entry_record(1).await.unwrap().unwrap().is_read);
    assert!(reopened.get_entry_record(2).await.unwrap().unwrap().is_starred);
}

#[wasm_bindgen_test]
async fn stale_settings_and_app_state_do_not_resurrect_deleted_entries_or_flags() {
    use rssr_infra::application_adapters::browser::adapters::BrowserAppStateAdapter;
    let first = seed_state(fixture()).await;
    let stale = BrowserStore::open().await.unwrap();
    let entries = BrowserEntryRepository::new(first);
    entries.set_read(1, true).await.unwrap();
    entries.delete_for_feed(1).await.unwrap();
    let settings = BrowserSettingsRepository::new(stale.clone());
    let draft = rssr_domain::UserSettings { refresh_interval_minutes: 15, ..Default::default() };
    settings.save(&draft).await.unwrap();
    BrowserAppStateAdapter::new(stale).save_snapshot(&Default::default()).await.unwrap();
    let snapshot = persisted_state().await;
    assert!(snapshot.core.entries.is_empty());
    assert!(snapshot.entry_flags.entries.is_empty());
    assert!(snapshot.entry_content.entries.is_empty());
    assert_eq!(snapshot.core.settings.refresh_interval_minutes, 15);
}

#[wasm_bindgen_test]
async fn concurrent_subscription_inserts_allocate_distinct_ids_and_bulk_preview_is_rechecked() {
    use rssr_domain::{EntryQuery, MarkReadOutcome, NewFeedSubscription};
    let first = seed_state(fixture()).await;
    let second = BrowserStore::open().await.unwrap();
    let a = BrowserFeedRepository::new(first.clone());
    let b = BrowserFeedRepository::new(second.clone());
    let subscription = |url| NewFeedSubscription {
        url: url::Url::parse(url).unwrap(),
        title: None,
        folder: None,
        site_url: None,
    };
    let one = subscription("https://one.example/rss");
    let two = subscription("https://two.example/rss");
    let (one, two) = tokio::join!(a.upsert_subscription(&one), b.upsert_subscription(&two));
    assert_ne!(one.unwrap().id, two.unwrap().id);
    assert_eq!(a.list_feeds().await.unwrap().len(), 3);
    let a = BrowserEntryRepository::new(first);
    let b = BrowserEntryRepository::new(second);
    let preview = a.preview_mark_read(&EntryQuery::default()).await.unwrap();
    b.set_read(1, true).await.unwrap();
    assert!(matches!(
        a.mark_read_if_unchanged(&preview).await.unwrap(),
        MarkReadOutcome::SelectionChanged { .. }
    ));
    assert!(!b.get_entry_record(2).await.unwrap().unwrap().is_read);
}

#[wasm_bindgen_test]
async fn flag_writes_and_warm_reads_do_not_read_or_write_article_bodies() {
    let state = seed_state(fixture()).await;
    let entries = BrowserEntryRepository::new(state);
    js_sys::eval(r#"
        globalThis.__storageCalls = [];
        globalThis.__get = Storage.prototype.getItem;
        globalThis.__set = Storage.prototype.setItem;
        Storage.prototype.getItem = function(k) { __storageCalls.push(['get', k]); return __get.call(this, k); };
        Storage.prototype.setItem = function(k,v) { __storageCalls.push(['set', k]); return __set.call(this, k,v); };
    "#).unwrap();
    let write = entries.set_read(1, true).await;
    let read = entries.get_entry_record(1).await;
    let calls = js_sys::eval(
        r#"
        Storage.prototype.getItem = __get; Storage.prototype.setItem = __set;
        delete globalThis.__get; delete globalThis.__set;
        JSON.stringify(__storageCalls);
    "#,
    )
    .unwrap()
    .as_string()
    .unwrap();
    write.unwrap();
    assert!(read.unwrap().unwrap().is_read);
    assert!(!calls.contains("entry-content"), "{calls}");
    assert!(!calls.contains("web-state-v1"), "{calls}");
}

#[wasm_bindgen_test]
async fn resetting_storage_changes_epoch_and_invalidates_existing_caches() {
    let existing = seed_state(fixture()).await;
    let mut replacement = fixture();
    replacement.core.entries[0].title = "Replacement after clearing storage".into();
    seed_state(replacement).await;
    let record = BrowserEntryRepository::new(existing).get_entry_record(1).await.unwrap().unwrap();
    assert_eq!(record.title, "Replacement after clearing storage");
}

#[wasm_bindgen_test]
async fn corrupt_legacy_or_committed_data_is_preserved_and_never_reset_to_empty() {
    let state = seed_state(fixture()).await;
    let original = committed_slice(STORAGE_KEY).unwrap();
    storage().remove_item(COMMIT_STORAGE_KEY).unwrap();
    storage().set_item(ENTRY_FLAGS_STORAGE_KEY, "broken flags").unwrap();
    assert!(BrowserStore::open().await.is_err());
    assert_eq!(storage().get_item(STORAGE_KEY).unwrap().as_deref(), Some(original.as_str()));
    assert_eq!(
        storage().get_item(ENTRY_FLAGS_STORAGE_KEY).unwrap().as_deref(),
        Some("broken flags")
    );
    seed_state(fixture()).await;
    let head = storage().get_item(COMMIT_STORAGE_KEY).unwrap();
    storage().remove_item(ENTRY_FLAGS_STORAGE_KEY).unwrap();
    assert!(state.snapshot().await.is_err());
    assert_eq!(storage().get_item(COMMIT_STORAGE_KEY).unwrap(), head);
    // Repair permits recovery on the same handle; a failed reload cannot expose stale memory.
    storage().set_item(ENTRY_FLAGS_STORAGE_KEY, r#"{"entries":[]}"#).unwrap();
    assert_eq!(state.snapshot().await.unwrap().core.entries.len(), 2);
}

#[wasm_bindgen_test]
async fn orphan_staging_is_ignored_and_cleanup_failure_does_not_undo_a_commit() {
    let state = seed_state(fixture()).await;
    storage().set_item(&format!("{STORAGE_KEY}-next"), "interrupted staging").unwrap();
    assert_eq!(persisted_state().await.core.entries.len(), 2);
    js_sys::eval("globalThis.__remove = Storage.prototype.removeItem; Storage.prototype.removeItem = function() { throw new Error('cleanup failed'); };").unwrap();
    let entries = BrowserEntryRepository::new(state);
    let first = entries.set_read(1, true).await;
    let second = entries.set_starred(2, true).await;
    js_sys::eval("Storage.prototype.removeItem = __remove; delete globalThis.__remove;").unwrap();
    first.unwrap();
    second.unwrap();
    assert!(entries.get_entry_record(1).await.unwrap().unwrap().is_read);
    assert!(entries.get_entry_record(2).await.unwrap().unwrap().is_starred);
    entries.set_read(1, false).await.unwrap();
    assert!(storage().get_item(&format!("{STORAGE_KEY}-next")).unwrap().is_none());
    assert_eq!(persisted_state().await.core.entries.len(), 2);
}

#[wasm_bindgen_test]
async fn cancelled_queued_write_never_runs_after_lock_release() {
    use std::task::Poll;
    let entries = BrowserEntryRepository::new(seed_state(fixture()).await);
    eval_promise(r#"new Promise(ready => navigator.locks.request('rssr-browser-state-v1', () => new Promise(release => { globalThis.__release = release; ready(); })))"#).await;
    let mut pending = entries.set_read(1, true);
    std::future::poll_fn(|cx| {
        assert!(pending.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    })
    .await;
    eval_promise("new Promise(resolve => setTimeout(resolve, 20))").await;
    drop(pending);
    js_sys::eval("__release(); delete globalThis.__release;").unwrap();
    assert!(!entries.get_entry_record(1).await.unwrap().unwrap().is_read);
}

#[wasm_bindgen_test]
async fn lock_timeout_and_missing_capability_fail_without_writing() {
    let entries = BrowserEntryRepository::new(seed_state(fixture()).await);
    eval_promise(r#"new Promise(ready => navigator.locks.request('rssr-browser-state-v1', () => new Promise(release => { globalThis.__release = release; ready(); })))"#).await;
    let failed = entries.set_read(1, true).await;
    js_sys::eval("__release(); delete globalThis.__release;").unwrap();
    assert!(failed.unwrap_err().to_string().contains("5 秒"));
    assert!(!entries.get_entry_record(1).await.unwrap().unwrap().is_read);
    js_sys::eval(
        "Object.defineProperty(navigator, 'locks', { value: undefined, configurable: true });",
    )
    .unwrap();
    let missing = entries.set_starred(1, true).await;
    js_sys::eval("delete navigator.locks;").unwrap();
    assert!(missing.is_err());
    assert!(!entries.get_entry_record(1).await.unwrap().unwrap().is_starred);
}

#[wasm_bindgen_test]
async fn failed_rollback_keeps_cache_unavailable_until_committed_data_can_be_reloaded() {
    let entries = BrowserEntryRepository::new(seed_state(fixture()).await);
    js_sys::eval(
        r#"
        globalThis.__get = Storage.prototype.getItem;
        globalThis.__set = Storage.prototype.setItem;
        globalThis.__denyRead = false;
        Storage.prototype.setItem = function() {
            __denyRead = true;
            throw new DOMException('quota', 'QuotaExceededError');
        };
        Storage.prototype.getItem = function(k) {
            if (__denyRead) throw new DOMException('denied', 'SecurityError');
            return __get.call(this,k);
        };
    "#,
    )
    .unwrap();
    let write = entries.set_read(1, true).await;
    let read = entries.get_entry_record(1).await;
    let open = BrowserStore::open().await;
    js_sys::eval(
        r#"
        Storage.prototype.getItem = __get; Storage.prototype.setItem = __set;
        delete globalThis.__get; delete globalThis.__set; delete globalThis.__denyRead;
    "#,
    )
    .unwrap();
    assert!(write.is_err());
    assert!(read.is_err());
    assert!(open.is_err());
    assert!(!entries.get_entry_record(1).await.unwrap().unwrap().is_read);
    entries.set_read(1, true).await.unwrap();
    assert!(entries.get_entry_record(1).await.unwrap().unwrap().is_read);
}

#[wasm_bindgen_test]
async fn missing_head_with_newer_slots_is_not_mistaken_for_an_empty_legacy_database() {
    let entries = BrowserEntryRepository::new(seed_state(fixture()).await);
    entries.set_starred(1, true).await.unwrap();
    let key = format!("{ENTRY_FLAGS_STORAGE_KEY}-next");
    let saved = storage().get_item(&key).unwrap().unwrap();
    storage().remove_item(COMMIT_STORAGE_KEY).unwrap();
    assert!(BrowserStore::open().await.is_err());
    assert!(entries.get_entry_record(1).await.is_err());
    assert_eq!(storage().get_item(&key).unwrap().as_deref(), Some(saved.as_str()));
    assert!(storage().get_item(ENTRY_FLAGS_STORAGE_KEY).unwrap().is_none());
    assert!(storage().get_item(COMMIT_STORAGE_KEY).unwrap().is_none());
}
