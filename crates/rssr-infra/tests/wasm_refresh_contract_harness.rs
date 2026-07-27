#![cfg(target_arch = "wasm32")]

use std::sync::{Arc, Mutex};

use reqwest::StatusCode;
use rssr_application::{
    FeedRefreshSourceOutput, FeedRefreshUpdate, ParsedEntryData, ParsedFeedUpdate, RefreshCommit,
    RefreshFailure, RefreshHttpMetadata, RefreshStorePort,
};
use rssr_infra::application_adapters::browser::{
    adapters::{
        BrowserRefreshStore, classify_browser_refresh_body, classify_browser_refresh_status,
    },
    state::{
        APP_STATE_STORAGE_KEY, BrowserState, ENTRY_FLAGS_STORAGE_KEY, LoadedState, PersistedFeed,
        PersistedState, STORAGE_KEY, load_state,
    },
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

const SAMPLE_FEED_XML: &str = r#"
<rss version="2.0">
  <channel>
    <title>Browser Source Feed</title>
    <link>https://example.com/</link>
    <description>Browser source fixture.</description>
    <item>
      <guid>entry-1</guid>
      <title>Browser Source Entry</title>
      <link>https://example.com/articles/entry-1</link>
      <description><![CDATA[<p>Browser source entry body.</p>]]></description>
    </item>
  </channel>
</rss>
"#;

#[wasm_bindgen_test]
fn browser_refresh_source_classifies_not_modified_status() {
    let output = classify_browser_refresh_status(
        StatusCode::NOT_MODIFIED,
        RefreshHttpMetadata {
            etag: Some("etag-not-modified".to_string()),
            last_modified: Some("Mon, 13 Apr 2026 09:00:00 GMT".to_string()),
        },
    );

    match output {
        Some(FeedRefreshSourceOutput::NotModified(metadata)) => {
            assert_eq!(metadata.etag.as_deref(), Some("etag-not-modified"));
        }
        other => panic!("unexpected source output: {other:?}"),
    }
}

#[wasm_bindgen_test]
fn browser_refresh_source_classifies_non_success_status_as_failure() {
    let output = classify_browser_refresh_status(
        StatusCode::FORBIDDEN,
        RefreshHttpMetadata {
            etag: Some("etag-forbidden".to_string()),
            last_modified: Some("Mon, 13 Apr 2026 09:30:00 GMT".to_string()),
        },
    );

    match output {
        Some(FeedRefreshSourceOutput::Failed(failure)) => {
            assert_eq!(failure.metadata.expect("metadata").etag.as_deref(), Some("etag-forbidden"));
            assert_eq!(failure.message, "feed 抓取返回非成功状态: HTTP status 403 Forbidden");
        }
        other => panic!("unexpected source output: {other:?}"),
    }
}

#[wasm_bindgen_test]
fn browser_refresh_source_allows_success_status_to_continue_to_body_classification() {
    let output = classify_browser_refresh_status(StatusCode::OK, RefreshHttpMetadata::default());

    assert!(output.is_none());
}

#[wasm_bindgen_test]
fn browser_refresh_source_classifies_valid_xml_body_as_updated() {
    let output = classify_browser_refresh_body(
        RefreshHttpMetadata {
            etag: Some("etag-source".to_string()),
            last_modified: Some("Mon, 13 Apr 2026 10:00:00 GMT".to_string()),
        },
        SAMPLE_FEED_XML,
    );

    match output {
        FeedRefreshSourceOutput::Updated(update) => {
            assert_eq!(update.metadata.etag.as_deref(), Some("etag-source"));
            assert_eq!(update.feed.title.as_deref(), Some("Browser Source Feed"));
            assert_eq!(update.feed.entries.len(), 1);
            assert_eq!(update.feed.entries[0].title, "Browser Source Entry");
        }
        other => panic!("unexpected source output: {other:?}"),
    }
}

#[wasm_bindgen_test]
fn browser_refresh_source_classifies_html_shell_body_as_parse_failure() {
    let output = classify_browser_refresh_body(
        RefreshHttpMetadata {
            etag: Some("etag-html".to_string()),
            last_modified: Some("Mon, 13 Apr 2026 11:00:00 GMT".to_string()),
        },
        "<!doctype html><html><body>login shell</body></html>",
    );

    match output {
        FeedRefreshSourceOutput::Failed(failure) => {
            assert_eq!(failure.metadata.expect("metadata").etag.as_deref(), Some("etag-html"));
            assert!(failure.message.starts_with("解析订阅失败:"));
            assert!(failure.message.contains("当前响应不是 XML feed"));
        }
        other => panic!("unexpected source output: {other:?}"),
    }
}

#[wasm_bindgen_test]
fn browser_refresh_source_classifies_bad_xml_body_as_parse_failure() {
    let output = classify_browser_refresh_body(
        RefreshHttpMetadata {
            etag: Some("etag-bad-xml".to_string()),
            last_modified: Some("Mon, 13 Apr 2026 12:00:00 GMT".to_string()),
        },
        "<?xml version=\"1.0\"?><rss><channel><item>",
    );

    match output {
        FeedRefreshSourceOutput::Failed(failure) => {
            assert_eq!(failure.metadata.expect("metadata").etag.as_deref(), Some("etag-bad-xml"));
            assert!(failure.message.starts_with("解析订阅失败:"));
        }
        other => panic!("unexpected source output: {other:?}"),
    }
}

#[wasm_bindgen_test]
async fn browser_refresh_store_lists_only_active_targets() {
    clear_browser_state_storage();

    let state = Arc::new(Mutex::new(BrowserState {
        core: PersistedState {
            next_feed_id: 2,
            feeds: vec![
                sample_feed(1, "https://example.com/feed-1.xml", false),
                sample_feed(2, "https://example.com/feed-2.xml", true),
            ],
            ..PersistedState::default()
        },
        ..BrowserState::default()
    }));
    let store = BrowserRefreshStore::new(state);

    let targets = store.list_targets().await.expect("list targets");

    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].feed_id, 1);
    assert_eq!(targets[0].url.as_str(), "https://example.com/feed-1.xml");

    clear_browser_state_storage();
}

#[wasm_bindgen_test]
async fn browser_refresh_store_get_target_normalizes_url_and_skips_deleted_feeds() {
    clear_browser_state_storage();

    let state = Arc::new(Mutex::new(BrowserState {
        core: PersistedState {
            next_feed_id: 2,
            feeds: vec![
                sample_feed(1, "https://example.com:443/feed.xml#frag", false),
                sample_feed(2, "https://example.com/deleted.xml", true),
            ],
            ..PersistedState::default()
        },
        ..BrowserState::default()
    }));
    let store = BrowserRefreshStore::new(state);

    let active = store.get_target(1).await.expect("get active target").expect("target exists");
    assert_eq!(active.feed_id, 1);
    assert_eq!(active.url.as_str(), "https://example.com/feed.xml");

    let deleted = store.get_target(2).await.expect("get deleted target");
    assert!(deleted.is_none());

    let missing = store.get_target(99).await.expect("get missing target");
    assert!(missing.is_none());

    clear_browser_state_storage();
}

#[wasm_bindgen_test]
async fn browser_refresh_store_commit_not_modified_updates_state_and_storage() {
    clear_browser_state_storage();

    let state = Arc::new(Mutex::new(BrowserState {
        core: PersistedState {
            next_feed_id: 1,
            feeds: vec![sample_feed(1, "https://example.com/feed.xml", false)],
            ..PersistedState::default()
        },
        ..BrowserState::default()
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
        assert_eq!(snapshot.core.feeds.len(), 1);
        assert_eq!(snapshot.core.feeds[0].etag.as_deref(), Some("etag-1"));
        assert_eq!(
            snapshot.core.feeds[0].last_modified.as_deref(),
            Some("Wed, 01 Apr 2026 10:00:00 GMT")
        );
        assert!(snapshot.core.feeds[0].last_fetched_at.is_some());
        assert!(snapshot.core.feeds[0].last_success_at.is_some());
        assert_eq!(snapshot.core.feeds[0].fetch_error, None);
    }

    let LoadedState { state: persisted, warning } = load_state();
    assert!(warning.is_none());
    assert_eq!(persisted.core.feeds.len(), 1);
    assert_eq!(persisted.core.feeds[0].etag.as_deref(), Some("etag-1"));

    clear_browser_state_storage();
}

#[wasm_bindgen_test]
async fn browser_refresh_store_commit_updated_persists_feed_metadata_and_entries() {
    clear_browser_state_storage();

    let state = Arc::new(Mutex::new(BrowserState {
        core: PersistedState {
            next_feed_id: 1,
            feeds: vec![sample_feed(1, "https://example.com/feed.xml", false)],
            ..PersistedState::default()
        },
        ..BrowserState::default()
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
        assert_eq!(snapshot.core.feeds.len(), 1);
        assert_eq!(snapshot.core.feeds[0].title.as_deref(), Some("Updated Feed"));
        assert_eq!(snapshot.core.feeds[0].site_url.as_deref(), Some("https://example.com/"));
        assert_eq!(snapshot.core.feeds[0].description.as_deref(), Some("Updated description"));
        assert_eq!(snapshot.core.feeds[0].etag.as_deref(), Some("etag-updated"));
        assert!(snapshot.core.feeds[0].last_fetched_at.is_some());
        assert!(snapshot.core.feeds[0].last_success_at.is_some());
        assert_eq!(snapshot.core.feeds[0].fetch_error, None);
        assert_eq!(snapshot.core.entries.len(), 2);
        assert_eq!(snapshot.core.entries[0].feed_id, 1);
        assert_eq!(snapshot.core.entries[0].title, "Entry 1");
        assert_eq!(snapshot.core.entries[1].title, "Entry 2");
    }

    let LoadedState { state: persisted, warning } = load_state();
    assert!(warning.is_none());
    assert_eq!(persisted.core.entries.len(), 2);
    assert_eq!(persisted.core.feeds[0].title.as_deref(), Some("Updated Feed"));

    clear_browser_state_storage();
}

#[wasm_bindgen_test]
async fn browser_refresh_store_commit_updated_clears_previous_fetch_error() {
    clear_browser_state_storage();

    let mut feed = sample_feed(1, "https://example.com/feed.xml", false);
    feed.fetch_error = Some("previous failure".to_string());

    let state = Arc::new(Mutex::new(BrowserState {
        core: PersistedState { next_feed_id: 1, feeds: vec![feed], ..PersistedState::default() },
        ..BrowserState::default()
    }));
    let store = BrowserRefreshStore::new(state.clone());

    store
        .commit(
            1,
            RefreshCommit::Updated {
                update: FeedRefreshUpdate {
                    metadata: RefreshHttpMetadata::default(),
                    feed: ParsedFeedUpdate {
                        title: Some("Recovered Feed".to_string()),
                        site_url: None,
                        description: None,
                        entries: vec![sample_entry(1)],
                    },
                },
            },
        )
        .await
        .expect("commit updated");

    let snapshot = state.lock().expect("lock state");
    assert_eq!(snapshot.core.feeds[0].fetch_error, None);

    clear_browser_state_storage();
}

#[wasm_bindgen_test]
async fn browser_refresh_store_commit_failed_persists_error_without_success_timestamp() {
    clear_browser_state_storage();

    let state = Arc::new(Mutex::new(BrowserState {
        core: PersistedState {
            next_feed_id: 1,
            feeds: vec![sample_feed(1, "https://example.com/feed.xml", false)],
            ..PersistedState::default()
        },
        ..BrowserState::default()
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
        assert_eq!(snapshot.core.feeds[0].etag.as_deref(), Some("etag-failed"));
        assert_eq!(
            snapshot.core.feeds[0].last_modified.as_deref(),
            Some("Fri, 03 Apr 2026 10:00:00 GMT")
        );
        assert!(snapshot.core.feeds[0].last_fetched_at.is_some());
        assert!(snapshot.core.feeds[0].last_success_at.is_none());
        assert_eq!(snapshot.core.feeds[0].fetch_error.as_deref(), Some("network timeout"));
        assert!(snapshot.core.entries.is_empty());
    }

    let LoadedState { state: persisted, warning } = load_state();
    assert!(warning.is_none());
    assert_eq!(persisted.core.feeds[0].fetch_error.as_deref(), Some("network timeout"));

    clear_browser_state_storage();
}

#[wasm_bindgen_test]
async fn browser_refresh_store_commit_failed_preserves_previous_success_timestamp() {
    clear_browser_state_storage();

    let mut feed = sample_feed(1, "https://example.com/feed.xml", false);
    let previous_success = OffsetDateTime::UNIX_EPOCH + time::Duration::days(3);
    let previous_fetch = OffsetDateTime::UNIX_EPOCH + time::Duration::days(4);
    feed.last_success_at = Some(previous_success);
    feed.last_fetched_at = Some(previous_fetch);

    let state = Arc::new(Mutex::new(BrowserState {
        core: PersistedState { next_feed_id: 1, feeds: vec![feed], ..PersistedState::default() },
        ..BrowserState::default()
    }));
    let store = BrowserRefreshStore::new(state.clone());

    store
        .commit(
            1,
            RefreshCommit::Failed {
                failure: RefreshFailure { message: "still failing".to_string(), metadata: None },
            },
        )
        .await
        .expect("commit failed");

    let snapshot = state.lock().expect("lock state");
    assert_eq!(snapshot.core.feeds[0].last_success_at, Some(previous_success));
    assert_ne!(snapshot.core.feeds[0].last_fetched_at, Some(previous_fetch));
    assert_eq!(snapshot.core.feeds[0].fetch_error.as_deref(), Some("still failing"));

    clear_browser_state_storage();
}

fn store_with_one_feed() -> (Arc<Mutex<BrowserState>>, BrowserRefreshStore) {
    let state = Arc::new(Mutex::new(BrowserState {
        core: PersistedState {
            next_feed_id: 1,
            feeds: vec![sample_feed(1, "https://example.com/feed.xml", false)],
            ..PersistedState::default()
        },
        ..BrowserState::default()
    }));
    let store = BrowserRefreshStore::new(state.clone());
    (state, store)
}

fn not_modified_with_etag(etag: &str) -> RefreshCommit {
    RefreshCommit::NotModified {
        metadata: RefreshHttpMetadata { etag: Some(etag.to_string()), last_modified: None },
    }
}

/// 批次内的 `commit` 立刻对内存可见，但不落盘；`end_batch` 才写一次。
///
/// 这是整个批次机制的意义所在：`save_state_snapshot` 每次都要把全库重新序列化写回
/// `localStorage`，而这发生在主线程上。逐个订阅写会让刷新期间页面按订阅数成比例卡顿。
/// 「内存立刻可见」这一半同样要断言——页面读的是同一份共享内存状态，
/// 推迟落盘不能让刷新中途的界面读到旧值。
#[wasm_bindgen_test]
async fn browser_refresh_store_defers_the_write_until_the_batch_ends() {
    clear_browser_state_storage();
    let (state, store) = store_with_one_feed();

    store.begin_batch().await.expect("begin batch");
    store.commit(1, not_modified_with_etag("etag-batched")).await.expect("commit in batch");

    assert_eq!(
        state.lock().expect("lock state").core.feeds[0].etag.as_deref(),
        Some("etag-batched"),
        "批次内的改动必须立刻对内存可见"
    );
    assert!(
        load_state().state.core.feeds.is_empty(),
        "批次没结束就不该写 localStorage——这正是省下的那些整片重写"
    );

    store.end_batch().await.expect("end batch");

    let LoadedState { state: persisted, warning } = load_state();
    assert!(warning.is_none());
    assert_eq!(persisted.core.feeds[0].etag.as_deref(), Some("etag-batched"));

    clear_browser_state_storage();
}

/// 空批次不产生写入。
///
/// 整轮所有订阅都返回 304 是常态，那种情况下不该白白整片重写一次全库。
#[wasm_bindgen_test]
async fn browser_refresh_store_batch_without_commits_writes_nothing() {
    clear_browser_state_storage();
    let (_state, store) = store_with_one_feed();

    store.begin_batch().await.expect("begin batch");
    store.end_batch().await.expect("end batch");

    assert!(load_state().state.core.feeds.is_empty(), "没有 commit 的批次不该产生写入");

    clear_browser_state_storage();
}

/// 漏掉 `end_batch` 时，下一次 `begin_batch` 要把上一批改动落盘而不是丢掉。
///
/// 这是批次机制的安全网：写入推迟意味着「有人忘了关批次」会变成丢数据，
/// 因此重开批次必须先冲掉上一批。最坏退化成「晚一轮才写」，而不是永久丢失。
#[wasm_bindgen_test]
async fn browser_refresh_store_reopening_a_batch_flushes_the_unclosed_one() {
    clear_browser_state_storage();
    let (_state, store) = store_with_one_feed();

    store.begin_batch().await.expect("begin batch");
    store.commit(1, not_modified_with_etag("etag-orphaned")).await.expect("commit in batch");
    // 故意不调用 end_batch，直接开下一轮。
    store.begin_batch().await.expect("reopen batch");

    assert_eq!(
        load_state().state.core.feeds[0].etag.as_deref(),
        Some("etag-orphaned"),
        "重开批次必须先把上一批没关掉的改动写下去"
    );

    store.end_batch().await.expect("end batch");
    clear_browser_state_storage();
}

/// 批次外的 `commit` 行为不变：立刻落盘。
///
/// `refresh_all` 之外还有单订阅刷新与添加订阅时的提交，它们不开批次。
/// 「不开批次就是原来的行为」是这次改动不影响那些路径的依据。
#[wasm_bindgen_test]
async fn browser_refresh_store_commit_outside_a_batch_still_writes_immediately() {
    clear_browser_state_storage();
    let (_state, store) = store_with_one_feed();

    store.commit(1, not_modified_with_etag("etag-unbatched")).await.expect("commit outside batch");

    assert_eq!(
        load_state().state.core.feeds[0].etag.as_deref(),
        Some("etag-unbatched"),
        "没有批次时 commit 必须像改动前一样立刻落盘"
    );

    clear_browser_state_storage();
}

/// 批次被中断后，存储必须回到「立刻落盘」的正常状态。
///
/// Web 端刷新任务绑定在订阅页组件作用域上，用户刷新途中切走页面，整个 future 就被 drop，
/// `end_batch` 永远等不到，由批次守卫的析构调用 `abort_batch` 兜底。
/// 少了这条兜底，`active` 会永久停在 true：此后单订阅刷新与添加订阅的首刷都只改内存不落盘，
/// 用户看到的是「刷新按钮没反应」，刷新页面后条目全空——而且全程不报错。
#[wasm_bindgen_test]
async fn browser_refresh_store_abort_flushes_and_restores_immediate_writes() {
    clear_browser_state_storage();
    let (_state, store) = store_with_one_feed();

    store.begin_batch().await.expect("begin batch");
    store.commit(1, not_modified_with_etag("etag-interrupted")).await.expect("commit in batch");

    // 等价于刷新 future 在这里被取消：守卫析构调用 abort_batch。
    store.abort_batch();

    assert_eq!(
        load_state().state.core.feeds[0].etag.as_deref(),
        Some("etag-interrupted"),
        "中断时已经抓到的改动要尽力落盘，而不是随批次一起丢掉"
    );

    store.commit(1, not_modified_with_etag("etag-after-abort")).await.expect("commit after abort");

    assert_eq!(
        load_state().state.core.feeds[0].etag.as_deref(),
        Some("etag-after-abort"),
        "批次被中断之后，后续的批次外提交不能继续被吞掉"
    );

    clear_browser_state_storage();
}

/// `abort_batch` 幂等：正常收尾后守卫析构还会再调一次，那一次必须什么都不写。
///
/// 守卫刻意不设「解除」开关（那会留出「已解除但还没 end_batch」的取消窗口），
/// 因此幂等性是它成立的前提。
#[wasm_bindgen_test]
async fn browser_refresh_store_abort_after_end_batch_writes_nothing() {
    clear_browser_state_storage();
    let (state, store) = store_with_one_feed();

    store.begin_batch().await.expect("begin batch");
    store.commit(1, not_modified_with_etag("etag-done")).await.expect("commit in batch");
    store.end_batch().await.expect("end batch");

    // 绕过 commit 直接改内存：abort 若真的又写了一次，这个值就会被带进 localStorage。
    state.lock().expect("lock state").core.feeds[0].title = Some("只在内存里".to_string());
    store.abort_batch();

    assert_eq!(
        load_state().state.core.feeds[0].title.as_deref(),
        Some("Feed 1"),
        "end_batch 之后的 abort 必须是空操作"
    );

    clear_browser_state_storage();
}
