#![cfg(not(target_arch = "wasm32"))]

//! 归档语义的跨实现一致性。
//!
//! 同一个 `ArchiveFilter` 有三处实现：`rssr_domain::is_entry_archived`（比较
//! `OffsetDateTime`）、SQLite 适配器（在 RFC3339 **字符串**上比较）、浏览器适配器
//! （比较 `OffsetDateTime`）。字符串比较只在「两侧精度一致」时才等价于时间比较，
//! 因此分界被截断到整秒；这个测试就是钉住那个前提。
//!
//! 曾经的真实故障：`now` 带纳秒 ⇒ 分界形如 `…45.179Z`，而条目是整秒 `…45Z`。
//! 逐字符比较时 `'Z'`(0x5A) > `'.'`(0x2E)，于是 SQL 判定「条目 >= 分界」（显示），
//! Rust 判定「条目更早」（已归档）——同一篇文章既出现在列表里，又被计入已归档数量。

use rssr_domain::{
    ArchiveFilter, EntryQuery, FeedRepository, NewFeedSubscription, archive_cutoff_at,
    is_entry_archived,
};
use rssr_infra::{
    db::{
        entry_repository::SqliteEntryRepository, feed_repository::SqliteFeedRepository, migrate,
        sqlite_native::NativeSqliteBackend, storage_backend::StorageBackend,
    },
    parser::feed_parser::ParsedEntry,
};
use time::OffsetDateTime;
use url::Url;

const ARCHIVE_AFTER_MONTHS: u32 = 3;

#[tokio::test]
async fn sql_archive_filter_agrees_with_is_entry_archived_at_the_second_boundary() {
    let backend = NativeSqliteBackend::new("sqlite::memory:");
    let pool = backend.connect().await.expect("connect sqlite memory");
    migrate(&pool).await.expect("run migrations");

    let feed_repository = SqliteFeedRepository::new(pool.clone());
    let entry_repository = SqliteEntryRepository::new(pool.clone());

    let feed = feed_repository
        .upsert_subscription(&NewFeedSubscription {
            url: Url::parse("https://example.com/parity.xml").expect("valid url"),
            title: None,
            folder: None,
        })
        .await
        .expect("create feed");

    // 刻意使用带纳秒的 now：生产里 `OffsetDateTime::now_utc()` 就是这样。
    let now = OffsetDateTime::now_utc();
    let cutoff = archive_cutoff_at(now, ARCHIVE_AFTER_MONTHS).expect("cutoff");
    assert_eq!(cutoff.nanosecond(), 0, "分界必须是整秒，否则字符串比较会与时间比较分叉");

    // 条目时间戳都是整秒：feed 解析走 `from_unix_timestamp`。
    let candidates =
        [cutoff - time::Duration::seconds(1), cutoff, cutoff + time::Duration::seconds(1)];

    let parsed = candidates
        .iter()
        .enumerate()
        .map(|(index, published_at)| ParsedEntry {
            external_id: format!("entry-{index}"),
            dedup_key: format!("entry-{index}"),
            url: Some(Url::parse(&format!("https://example.com/{index}")).expect("valid url")),
            title: format!("Entry {index}"),
            author: None,
            summary: Some("summary".to_string()),
            content_html: None,
            content_text: Some("summary".to_string()),
            published_at: Some(*published_at),
            updated_at_source: None,
        })
        .collect::<Vec<_>>();
    entry_repository.upsert_entries(feed.id, &parsed).await.expect("insert entries");

    let mut sql_visible = entry_repository
        .list_entries(&EntryQuery {
            archive_filter: ArchiveFilter::ExcludeArchived { cutoff },
            ..EntryQuery::default()
        })
        .await
        .expect("list non-archived")
        .into_iter()
        .map(|entry| entry.title)
        .collect::<Vec<_>>();
    sql_visible.sort();

    let mut domain_visible = candidates
        .iter()
        .enumerate()
        .filter(|(_, published_at)| {
            !is_entry_archived(Some(**published_at), ARCHIVE_AFTER_MONTHS, now)
        })
        .map(|(index, _)| format!("Entry {index}"))
        .collect::<Vec<_>>();
    domain_visible.sort();

    assert_eq!(
        sql_visible, domain_visible,
        "SQL 归档过滤与 is_entry_archived 必须对同一批条目给出同一答案（cutoff={cutoff}）"
    );

    // 分界当刻的条目应当算作已归档（严格小于才可见），并被计入已归档数量。
    let archived_count = entry_repository
        .count_entries(&EntryQuery {
            archive_filter: ArchiveFilter::OnlyArchived { cutoff },
            ..EntryQuery::default()
        })
        .await
        .expect("count archived");
    assert_eq!(archived_count as usize, candidates.len() - sql_visible.len());
}
