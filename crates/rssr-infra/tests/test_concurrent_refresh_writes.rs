#![cfg(not(target_arch = "wasm32"))]

//! 「刷新全部」并发度从 1 提到 4 之后的守护测试。
//!
//! SQLite 同一时刻只允许一个写者。多个刷新任务各自开启写事务批量写入 entries 时，
//! 如果 journal 模式或 busy_timeout 不合适，就会拿到 `SQLITE_BUSY`（database is locked），
//! 表现为刷新偶发失败。这个测试针对**文件型**数据库（不是 `:memory:`，后者连接池只有 1）
//! 跑并发写，确保并发刷新不会互相把对方顶掉。

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use rssr_domain::{EntryQuery, FeedRepository, NewFeedSubscription};
use rssr_infra::{
    db::{
        entry_repository::SqliteEntryRepository, feed_repository::SqliteFeedRepository,
        sqlite_native::NativeSqliteBackend, storage_backend::StorageBackend,
    },
    parser::feed_parser::ParsedEntry,
};
use url::Url;

const FEED_COUNT: i64 = 4;
const ENTRIES_PER_FEED: i64 = 60;

fn entry(feed_index: i64, index: i64) -> ParsedEntry {
    let key = format!("feed-{feed_index}-entry-{index}");
    ParsedEntry {
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
    let base_dir = std::env::temp_dir().join(format!("rssr-concurrent-refresh-{nonce}"));
    let database_path = base_dir.join("rss-reader.db");

    let backend = NativeSqliteBackend::with_path(&database_path);
    let index_pool = backend.connect().await.expect("connect index db");
    backend.migrate(&index_pool).await.expect("migrate index db");
    let content_pool = backend.connect_content().await.expect("connect content db");
    backend.migrate_content(&content_pool).await.expect("migrate content db");

    // 并发写能成立的前提是 WAL：回滚日志模式下第二个写者会直接吃到 SQLITE_BUSY。
    // 这里断言出来，是为了将来有人改 `SqliteConnectOptions` 时能立刻看到代价。
    let journal_mode: String = sqlx::query_scalar("PRAGMA journal_mode")
        .fetch_one(&index_pool)
        .await
        .expect("read journal_mode");
    assert_eq!(
        journal_mode.to_ascii_lowercase(),
        "wal",
        "并发刷新依赖 WAL 日志模式；改动连接选项前请先确认并发写仍然安全"
    );

    let feed_repository = SqliteFeedRepository::new(index_pool.clone());
    let entry_repository =
        Arc::new(SqliteEntryRepository::new_with_content_pool(index_pool.clone(), content_pool));

    let mut feed_ids = Vec::new();
    for feed_index in 0..FEED_COUNT {
        let feed = feed_repository
            .upsert_subscription(&NewFeedSubscription {
                url: Url::parse(&format!("https://example.com/feed-{feed_index}.xml"))
                    .expect("valid url"),
                title: Some(format!("Feed {feed_index}")),
                folder: None,
            })
            .await
            .expect("create feed");
        feed_ids.push((feed_index, feed.id));
    }

    // 与 REFRESH_ALL_CONCURRENCY = 4 对应：四个订阅同时提交各自的批量写入。
    let mut tasks = tokio::task::JoinSet::new();
    for (feed_index, feed_id) in feed_ids {
        let entry_repository = Arc::clone(&entry_repository);
        tasks.spawn(async move {
            let entries =
                (0..ENTRIES_PER_FEED).map(|index| entry(feed_index, index)).collect::<Vec<_>>();
            entry_repository.upsert_entries(feed_id, &entries).await
        });
    }

    while let Some(joined) = tasks.join_next().await {
        let written = joined.expect("refresh task should not panic");
        let written = written.expect("concurrent batch write should not fail with SQLITE_BUSY");
        assert_eq!(written as i64, ENTRIES_PER_FEED);
    }

    let total = entry_repository
        .count_entries(&EntryQuery::default())
        .await
        .expect("count entries after concurrent writes");
    assert_eq!(total as i64, FEED_COUNT * ENTRIES_PER_FEED);

    index_pool.close().await;

    let _ = std::fs::remove_dir_all(&base_dir);
}
