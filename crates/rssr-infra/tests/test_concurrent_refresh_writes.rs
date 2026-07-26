#![cfg(not(target_arch = "wasm32"))]

//! 「刷新全部」并发度从 1 提到 4 之后的守护测试。
//!
//! 真正起守护作用的是那条 `PRAGMA journal_mode` 断言。
//!
//! 并发写这一半**并不能**区分 WAL 与回滚日志：两种模式都设了 busy_timeout，而这里的写事务
//! 第一条语句就是写（不存在「先读后升级」），SQLite 的 busy handler 会正常介入让写者排队，
//! 因此回滚模式下这段大概也能通过。它验证的是「并发批量写不会互相顶掉」这件事本身。
//!
//! journal 模式的断言才是关键：WAL 决定了写者是否阻塞读者，而刷新并发度正是依据它决定的
//! （见 `rssr-app` 启动时的实测）。测试必须针对**文件型**数据库——`:memory:` 不支持 WAL，
//! 且连接池只有 1。

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
    let content_pool_handle = content_pool.clone();

    // 刷新并发度依据实际生效的 journal 模式决定，因此这里把它断言出来：
    // 有人改 `SqliteConnectOptions` 导致静默退回 `delete` 时，这条会立刻失败。
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
        // 只断言「没有失败」：`upsert_entries` 无条件返回 `entries.len()`，
        // 拿返回值和入参比是重言式，真正的验证是后面的 count_entries。
        written.expect("concurrent batch write should not fail with SQLITE_BUSY");
    }

    let total = entry_repository
        .count_entries(&EntryQuery::default())
        .await
        .expect("count entries after concurrent writes");
    assert_eq!(total as i64, FEED_COUNT * ENTRIES_PER_FEED);

    // 两个池都要关：只关 index 的话 content 库文件在 Windows 上仍被占用，
    // 临时目录连同 `-wal` / `-shm` 会一直堆积。
    index_pool.close().await;
    content_pool_handle.close().await;

    // 清理是尽力而为，**不做断言**：句柄释放到文件可删之间存在 OS 层的时间差，
    // 断言它会让这个测试变成偶发失败——而清理本身并不是这个测试要验证的行为。
    for _ in 0..10 {
        if std::fs::remove_dir_all(&base_dir).is_ok() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
}
