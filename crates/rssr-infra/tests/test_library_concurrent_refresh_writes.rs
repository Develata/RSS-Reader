#![cfg(not(target_arch = "wasm32"))]

//! Guard for "refresh all" running several subscriptions at once, run against the v0.2 library
//! store (ported from `test_concurrent_refresh_writes.rs`).
//!
//! The `journal_mode` assertion is the load-bearing one: WAL decides whether a writer blocks
//! readers, and the refresh concurrency is chosen on that basis. The test must use a *file*
//! library -- an in-memory database has no WAL and a single connection.
//!
//! The concurrent-write half cannot tell WAL from a rollback journal by itself; it checks that
//! concurrent batch writes do not push each other out. In the library, writes from one process
//! queue on the store's single writer connection instead of racing for SQLite's lock.

#[path = "support/library.rs"]
mod library;

use std::time::{SystemTime, UNIX_EPOCH};

use library::Library;
use rssr_application::{ParsedEntryData, RefreshStorePort};
use rssr_domain::{EntryIndexRepository, EntryQuery, FeedRepository, NewFeedSubscription};
use rssr_infra::store::LibraryStore;
use url::Url;

const FEED_COUNT: i64 = 4;
const ENTRIES_PER_FEED: i64 = 60;

fn entry(feed_index: i64, index: i64) -> ParsedEntryData {
    let key = format!("feed-{feed_index}-entry-{index}");
    ParsedEntryData {
        external_id: key.clone(),
        dedup_key: key.clone(),
        url: Some(Url::parse(&format!("https://example.com/{key}")).expect("valid url")),
        title: format!("Entry {key}"),
        author: None,
        summary: Some("summary".to_string()),
        content_html: Some(format!("<p>body {key}</p>")),
        content_text: Some("summary".to_string()),
        published_at: None,
        updated_at_source: None,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_feed_writes_do_not_hit_sqlite_busy() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after unix epoch")
        .as_nanos();
    let base_dir = std::env::temp_dir().join(format!("rssr-library-concurrent-refresh-{nonce}"));
    let database_path = base_dir.join("library.sqlite3");

    let store = LibraryStore::open(&database_path).await.expect("open file library");
    // 刷新并发度依据实际生效的 journal 模式决定，因此这里把它断言出来：
    // 有人改连接选项导致静默退回 `delete` 时，这条会立刻失败。
    assert_eq!(
        store.journal_mode().to_ascii_lowercase(),
        "wal",
        "并发刷新依赖 WAL 日志模式；改动连接选项前请先确认并发写仍然安全"
    );
    let journal_mode = store
        .read(|conn| Ok(conn.query_row("PRAGMA journal_mode", [], |row| row.get::<_, String>(0))?))
        .await
        .expect("read journal_mode");
    assert_eq!(journal_mode.to_ascii_lowercase(), "wal");

    let library = Library::over(store);
    let mut feed_ids = Vec::new();
    for feed_index in 0..FEED_COUNT {
        let feed = library
            .feeds
            .upsert_subscription(&NewFeedSubscription {
                site_url: None,
                url: Url::parse(&format!("https://example.com/feed-{feed_index}.xml"))
                    .expect("valid url"),
                title: Some(format!("Feed {feed_index}")),
                source_title: None,
                folder: None,
            })
            .await
            .expect("create feed");
        feed_ids.push((feed_index, feed.id));
    }

    // 与 REFRESH_ALL_CONCURRENCY = 4 对应：四个订阅同时提交各自的批量写入。
    let mut tasks = tokio::task::JoinSet::new();
    for (feed_index, feed_id) in feed_ids {
        let refresh = library.refresh.clone();
        tasks.spawn(async move {
            let entries =
                (0..ENTRIES_PER_FEED).map(|index| entry(feed_index, index)).collect::<Vec<_>>();
            let target = refresh.get_target(feed_id).await?.expect("active feed");
            refresh.commit(&target, library::updated(entries)).await
        });
    }

    while let Some(joined) = tasks.join_next().await {
        let written = joined.expect("refresh task should not panic");
        let outcome = written.expect("concurrent batch write should not fail with SQLITE_BUSY");
        assert_eq!(outcome.inserted_count, ENTRIES_PER_FEED as u64);
    }

    let total = library
        .entries
        .count_entries(&EntryQuery::default())
        .await
        .expect("count entries after concurrent writes");
    assert_eq!(total as i64, FEED_COUNT * ENTRIES_PER_FEED);

    // 关掉全部连接后再清理：Windows 上文件被占用时目录（连同 `-wal` / `-shm`）删不掉。
    drop(library);

    // 清理是尽力而为，**不做断言**：句柄释放到文件可删之间存在 OS 层的时间差，
    // 断言它会让这个测试变成偶发失败——而清理本身并不是这个测试要验证的行为。
    for _ in 0..10 {
        if std::fs::remove_dir_all(&base_dir).is_ok() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
}
