#![cfg(target_arch = "wasm32")]

use reqwest::StatusCode;
use rssr_application::{
    FeedRefreshSourceOutput, FeedRefreshUpdate, ParsedEntryData, ParsedFeedUpdate, RefreshCommit,
    RefreshFailure, RefreshHttpMetadata, RefreshStorePort,
};
use rssr_infra::application_adapters::browser::{
    adapters::{
        BrowserRefreshStore, classify_browser_refresh_body, classify_browser_refresh_status,
    },
    state::{BrowserState, BrowserStore, PersistedFeed, PersistedState},
};
use time::OffsetDateTime;
use url::Url;
use wasm_bindgen_test::wasm_bindgen_test;

wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

#[path = "support/browser_storage.rs"]
#[allow(dead_code)]
mod browser_storage;
use browser_storage::{clear_browser_state_storage, persisted_state, seed_state};

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

    let state = seed_state(BrowserState {
        core: PersistedState {
            next_feed_id: 2,
            feeds: vec![
                sample_feed(1, "https://example.com/feed-1.xml", false),
                sample_feed(2, "https://example.com/feed-2.xml", true),
            ],
            ..PersistedState::default()
        },
        ..BrowserState::default()
    })
    .await;
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

    let state = seed_state(BrowserState {
        core: PersistedState {
            next_feed_id: 2,
            feeds: vec![
                sample_feed(1, "https://example.com:443/feed.xml#frag", false),
                sample_feed(2, "https://example.com/deleted.xml", true),
            ],
            ..PersistedState::default()
        },
        ..BrowserState::default()
    })
    .await;
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

    let state = seed_state(BrowserState {
        core: PersistedState {
            next_feed_id: 1,
            feeds: vec![sample_feed(1, "https://example.com/feed.xml", false)],
            ..PersistedState::default()
        },
        ..BrowserState::default()
    })
    .await;
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
        let snapshot = state.snapshot().await.expect("snapshot");
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

    let persisted = persisted_state().await;
    assert_eq!(persisted.core.feeds.len(), 1);
    assert_eq!(persisted.core.feeds[0].etag.as_deref(), Some("etag-1"));

    clear_browser_state_storage();
}

#[wasm_bindgen_test]
async fn browser_refresh_store_commit_updated_persists_feed_metadata_and_entries() {
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
        let snapshot = state.snapshot().await.expect("snapshot");
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

    let persisted = persisted_state().await;
    assert_eq!(persisted.core.entries.len(), 2);
    assert_eq!(persisted.core.feeds[0].title.as_deref(), Some("Updated Feed"));

    clear_browser_state_storage();
}

#[wasm_bindgen_test]
async fn browser_refresh_store_commit_updated_clears_previous_fetch_error() {
    clear_browser_state_storage();

    let mut feed = sample_feed(1, "https://example.com/feed.xml", false);
    feed.fetch_error = Some("previous failure".to_string());

    let state = seed_state(BrowserState {
        core: PersistedState { next_feed_id: 1, feeds: vec![feed], ..PersistedState::default() },
        ..BrowserState::default()
    })
    .await;
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

    let snapshot = state.snapshot().await.expect("snapshot");
    assert_eq!(snapshot.core.feeds[0].fetch_error, None);

    clear_browser_state_storage();
}

#[wasm_bindgen_test]
async fn browser_refresh_store_commit_failed_persists_error_without_success_timestamp() {
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
        let snapshot = state.snapshot().await.expect("snapshot");
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

    let persisted = persisted_state().await;
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

    let state = seed_state(BrowserState {
        core: PersistedState { next_feed_id: 1, feeds: vec![feed], ..PersistedState::default() },
        ..BrowserState::default()
    })
    .await;
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

    let snapshot = state.snapshot().await.expect("snapshot");
    assert_eq!(snapshot.core.feeds[0].last_success_at, Some(previous_success));
    assert_ne!(snapshot.core.feeds[0].last_fetched_at, Some(previous_fetch));
    assert_eq!(snapshot.core.feeds[0].fetch_error.as_deref(), Some("still failing"));

    clear_browser_state_storage();
}

async fn store_with_one_feed() -> (BrowserStore, BrowserRefreshStore) {
    let state = seed_state(BrowserState {
        core: PersistedState {
            next_feed_id: 1,
            feeds: vec![sample_feed(1, "https://example.com/feed.xml", false)],
            ..PersistedState::default()
        },
        ..BrowserState::default()
    })
    .await;
    let store = BrowserRefreshStore::new(state.clone());
    (state, store)
}

fn not_modified_with_etag(etag: &str) -> RefreshCommit {
    RefreshCommit::NotModified {
        metadata: RefreshHttpMetadata { etag: Some(etag.to_string()), last_modified: None },
    }
}

/// 每个订阅提交返回前必须落盘，页面退出不应丢失已经报告成功的结果。
#[wasm_bindgen_test]
async fn browser_refresh_store_commits_are_durable_before_the_batch_ends() {
    clear_browser_state_storage();
    let (state, store) = store_with_one_feed().await;

    store.begin_batch().await.expect("begin batch");
    store.commit(1, not_modified_with_etag("etag-batched")).await.expect("commit in batch");

    assert_eq!(
        state.snapshot().await.expect("snapshot").core.feeds[0].etag.as_deref(),
        Some("etag-batched"),
        "批次内的改动必须立刻对内存可见"
    );
    assert_eq!(persisted_state().await.core.feeds[0].etag.as_deref(), Some("etag-batched"));

    store.end_batch().await.expect("end batch");

    let persisted = persisted_state().await;
    assert_eq!(persisted.core.feeds[0].etag.as_deref(), Some("etag-batched"));

    clear_browser_state_storage();
}

/// 空批次不发布新版本。
#[wasm_bindgen_test]
async fn browser_refresh_store_batch_without_commits_writes_nothing() {
    clear_browser_state_storage();
    let (_state, store) = store_with_one_feed().await;

    let before = browser_storage::storage()
        .get_item(rssr_infra::application_adapters::browser::state::COMMIT_STORAGE_KEY)
        .unwrap();
    store.begin_batch().await.expect("begin batch");
    store.end_batch().await.expect("end batch");
    assert_eq!(
        browser_storage::storage()
            .get_item(rssr_infra::application_adapters::browser::state::COMMIT_STORAGE_KEY)
            .unwrap(),
        before
    );

    clear_browser_state_storage();
}

/// 漏掉 end_batch 不影响已提交结果。
#[wasm_bindgen_test]
async fn browser_refresh_store_reopening_a_batch_preserves_durable_results() {
    clear_browser_state_storage();
    let (_state, store) = store_with_one_feed().await;

    store.begin_batch().await.expect("begin batch");
    store.commit(1, not_modified_with_etag("etag-orphaned")).await.expect("commit in batch");
    // 故意不调用 end_batch，直接开下一轮。
    store.begin_batch().await.expect("reopen batch");

    assert_eq!(
        persisted_state().await.core.feeds[0].etag.as_deref(),
        Some("etag-orphaned"),
        "重开批次保留上轮已提交结果"
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
    let (_state, store) = store_with_one_feed().await;

    store.commit(1, not_modified_with_etag("etag-unbatched")).await.expect("commit outside batch");

    assert_eq!(
        persisted_state().await.core.feeds[0].etag.as_deref(),
        Some("etag-unbatched"),
        "没有批次时 commit 必须像改动前一样立刻落盘"
    );

    clear_browser_state_storage();
}

/// 取消刷新保留此前成功提交，后续操作仍可正常落盘。
#[wasm_bindgen_test]
async fn browser_refresh_store_abort_preserves_commits_and_subsequent_writes() {
    clear_browser_state_storage();
    let (_state, store) = store_with_one_feed().await;

    store.begin_batch().await.expect("begin batch");
    store.commit(1, not_modified_with_etag("etag-interrupted")).await.expect("commit in batch");

    // 等价于刷新 future 在这里被取消：守卫析构调用 abort_batch。
    store.abort_batch();

    assert_eq!(
        persisted_state().await.core.feeds[0].etag.as_deref(),
        Some("etag-interrupted"),
        "中断不丢失已成功提交的改动"
    );

    store.commit(1, not_modified_with_etag("etag-after-abort")).await.expect("commit after abort");

    assert_eq!(
        persisted_state().await.core.feeds[0].etag.as_deref(),
        Some("etag-after-abort"),
        "批次被中断之后，后续的批次外提交不能继续被吞掉"
    );

    clear_browser_state_storage();
}

/// 正常收尾后重复 abort 也不发布新版本。
#[wasm_bindgen_test]
async fn browser_refresh_store_abort_after_end_batch_writes_nothing() {
    let (_state, store) = store_with_one_feed().await;
    store.begin_batch().await.unwrap();
    store.commit(1, not_modified_with_etag("etag-done")).await.unwrap();
    store.end_batch().await.unwrap();
    let before = browser_storage::storage()
        .get_item(rssr_infra::application_adapters::browser::state::COMMIT_STORAGE_KEY)
        .unwrap();
    store.abort_batch();
    store.abort_batch();
    assert_eq!(
        browser_storage::storage()
            .get_item(rssr_infra::application_adapters::browser::state::COMMIT_STORAGE_KEY)
            .unwrap(),
        before
    );
    clear_browser_state_storage();
}

#[path = "support/refresh_count_cases.rs"]
mod refresh_count_cases;
#[wasm_bindgen_test]
async fn browser_counts_only_real_inserts_including_same_batch_duplicates() {
    clear_browser_state_storage();
    let state = seed_state(BrowserState {
        core: PersistedState {
            next_feed_id: 1,
            feeds: vec![sample_feed(1, "https://example.com/count", false)],
            ..Default::default()
        },
        ..Default::default()
    })
    .await;
    let store = BrowserRefreshStore::new(state.clone());
    refresh_count_cases::verify_counts(&store, 1).await;
    assert_eq!(state.snapshot().await.unwrap().core.entries.len(), 3);
    let persisted = persisted_state().await;
    assert_eq!(persisted.core.entries.len(), 3);
    clear_browser_state_storage();
}

#[wasm_bindgen_test]
async fn failed_refresh_at_each_publication_stage_keeps_old_content_and_index() {
    for fail_at in 1..=3 {
        let (state, store) = store_with_one_feed().await;
        let before = serde_json::to_value(&state.snapshot().await.unwrap().core).unwrap();
        js_sys::eval(&format!(r#"
            globalThis.__set = Storage.prototype.setItem;
            globalThis.__writes = 0;
            Storage.prototype.setItem = function(k,v) {{
                if (++globalThis.__writes === {fail_at}) throw new DOMException('quota', 'QuotaExceededError');
                return __set.call(this,k,v);
            }};
        "#)).unwrap();
        let result = store
            .commit(
                1,
                RefreshCommit::Updated {
                    update: FeedRefreshUpdate {
                        metadata: RefreshHttpMetadata::default(),
                        feed: ParsedFeedUpdate {
                            title: Some("New title".into()),
                            site_url: None,
                            description: None,
                            entries: vec![sample_entry(1)],
                        },
                    },
                },
            )
            .await;
        js_sys::eval("Storage.prototype.setItem = __set; delete globalThis.__set;").unwrap();
        assert!(result.is_err(), "stage {fail_at} must not report inserted_count success");
        assert_eq!(serde_json::to_value(&state.snapshot().await.unwrap().core).unwrap(), before);
        let persisted = persisted_state().await;
        assert_eq!(serde_json::to_value(&persisted.core).unwrap(), before);
        assert!(persisted.entry_content.entries.is_empty());
        // Orphan staged slices from the failed transaction must not affect the next commit.
        store.commit(1, not_modified_with_etag("recovered")).await.unwrap();
        assert_eq!(persisted_state().await.core.feeds[0].etag.as_deref(), Some("recovered"));
        assert!(persisted_state().await.core.entries.is_empty());
    }
}

#[wasm_bindgen_test]
async fn refresh_after_other_tab_deletes_feed_does_not_resurrect_it() {
    use rssr_domain::FeedRepository;
    use rssr_infra::application_adapters::browser::adapters::BrowserFeedRepository;
    let (_, store) = store_with_one_feed().await;
    assert!(store.get_target(1).await.unwrap().is_some());
    BrowserFeedRepository::new(BrowserStore::open().await.unwrap())
        .set_deleted(1, true)
        .await
        .unwrap();
    assert!(store.commit(1, not_modified_with_etag("late response")).await.is_err());
    assert!(persisted_state().await.core.feeds[0].is_deleted);
}

#[wasm_bindgen_test]
async fn metadata_only_refreshes_do_not_rewrite_flags_or_bodies() {
    let mut fixture = BrowserState::default();
    fixture.core.next_feed_id = 20;
    fixture.core.feeds =
        (1..=20).map(|id| sample_feed(id, &format!("https://example.com/{id}"), false)).collect();
    let store = BrowserRefreshStore::new(seed_state(fixture).await);
    js_sys::eval(r#"
        globalThis.__set = Storage.prototype.setItem;
        globalThis.__writes = [];
        Storage.prototype.setItem = function(k,v) { __writes.push([k,v.length]); return __set.call(this,k,v); };
    "#).unwrap();
    let start = js_sys::Date::now();
    let mut results = Vec::new();
    for id in 1..=20 {
        results.push(store.commit(id, not_modified_with_etag("unchanged")).await);
    }
    let elapsed = js_sys::Date::now() - start;
    let writes = js_sys::eval(
        r#"
        Storage.prototype.setItem = __set; delete globalThis.__set;
        JSON.stringify(__writes);
    "#,
    )
    .unwrap()
    .as_string()
    .unwrap();
    for result in results {
        result.unwrap();
    }
    assert!(!writes.contains("entry-content"));
    assert!(!writes.contains("entry-flags"));
    let writes: Vec<(String, usize)> = serde_json::from_str(&writes).unwrap();
    assert_eq!(writes.len(), 40); // Core plus commit head per successful subscription.
    wasm_bindgen_test::console_log!(
        "20 metadata commits: {} ms, {} chars written",
        elapsed,
        writes.iter().map(|(_, len)| len).sum::<usize>()
    );
}

#[wasm_bindgen_test]
async fn multiple_content_commits_preserve_all_feeds_and_report_write_volume() {
    let mut fixture = BrowserState::default();
    fixture.core.next_feed_id = 12;
    fixture.core.feeds =
        (1..=12).map(|id| sample_feed(id, &format!("https://example.com/{id}"), false)).collect();
    let store = BrowserRefreshStore::new(seed_state(fixture).await);
    js_sys::eval(r#"
        globalThis.__set = Storage.prototype.setItem;
        globalThis.__writes = [];
        Storage.prototype.setItem = function(k,v) { __writes.push([k,v.length]); return __set.call(this,k,v); };
    "#).unwrap();
    let start = js_sys::Date::now();
    let mut results = Vec::new();
    for id in 1..=12 {
        let entries = (1..=10)
            .map(|index| {
                let mut entry = sample_entry(index);
                entry.content_html = Some(format!("<p>{}</p>", "x".repeat(2048)));
                entry.content_text = Some("x".repeat(2048));
                entry
            })
            .collect();
        results.push(
            store
                .commit(
                    id,
                    RefreshCommit::Updated {
                        update: FeedRefreshUpdate {
                            metadata: RefreshHttpMetadata::default(),
                            feed: ParsedFeedUpdate {
                                title: None,
                                site_url: None,
                                description: None,
                                entries,
                            },
                        },
                    },
                )
                .await,
        );
    }
    let elapsed = js_sys::Date::now() - start;
    let raw = js_sys::eval(
        r#"
        Storage.prototype.setItem = __set; delete globalThis.__set;
        JSON.stringify(__writes);
    "#,
    )
    .unwrap()
    .as_string()
    .unwrap();
    for result in results {
        assert_eq!(result.unwrap().inserted_count, 10);
    }
    assert!(!raw.contains("entry-flags"));
    let writes: Vec<(String, usize)> = serde_json::from_str(&raw).unwrap();
    assert_eq!(writes.len(), 36);
    let state = persisted_state().await;
    assert_eq!(state.core.entries.len(), 120);
    assert_eq!(state.entry_content.entries.len(), 120);
    wasm_bindgen_test::console_log!(
        "12 content commits, 120 articles (2 KiB HTML + 2 KiB text): {} ms, {} chars written; final core+body size {} bytes",
        elapsed,
        writes.iter().map(|(_, len)| len).sum::<usize>(),
        serde_json::to_string(&state.core).unwrap().len()
            + serde_json::to_string(&state.entry_content).unwrap().len()
    );
}
