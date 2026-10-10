#![cfg(not(target_arch = "wasm32"))]

//! 标题搜索在原生端按 Unicode 小写比较，与浏览器端（同一个 `rssr_domain::title_contains`）
//! 匹配到同样的文章（审计 D6）。SQLite 的 LIKE 只折叠 ASCII，原先 `école` 搜不到 `ÉCOLE`。

use std::collections::BTreeSet;

use rssr_domain::{
    EntryQuery, FeedRepository, NewFeedSubscription, fold_for_search, title_contains,
};
use rssr_infra::{
    db::{
        SqlitePool, entry_repository::SqliteEntryRepository, feed_repository::SqliteFeedRepository,
        migrate, sqlite_native::NativeSqliteBackend, storage_backend::StorageBackend,
    },
    parser::feed_parser::ParsedEntry,
};
use url::Url;

const TITLES: [&str; 8] = [
    "ÉCOLE de Paris",
    "Привет, МИР",
    "ΣΟΦΙΑ και φιλοσοφία",
    "Straße und Weg",
    "Plain ASCII Title",
    "中文标题",
    "100% Pure_Data",
    "abc\u{0}ÉCOLE",
];

fn searches() -> Vec<String> {
    let mut searches: Vec<String> =
        ["école", "мир", "σοφια", "STRASSE", "straße", "title", "标题", "0% p", "e_d"]
            .into_iter()
            .map(String::from)
            .collect();
    // NUL 结束 LIKE 的匹配；`İ` 折叠后变长，超过 LIKE 的模式长度上限（独立评审）。
    searches.push("\u{0}missing".into());
    searches.push("İ".repeat(16_667));
    searches
}

fn entry(index: usize, title: &str) -> ParsedEntry {
    ParsedEntry {
        external_id: format!("entry-{index}"),
        dedup_key: format!("entry-{index}"),
        url: None,
        title: title.to_string(),
        author: None,
        summary: None,
        content_html: None,
        content_text: None,
        published_at: None,
        updated_at_source: None,
    }
}

async fn seeded(titles: &[String]) -> (SqlitePool, SqliteEntryRepository, i64) {
    let pool = NativeSqliteBackend::new("sqlite::memory:").connect().await.expect("connect");
    migrate(&pool).await.expect("migrate");
    let feeds = SqliteFeedRepository::new(pool.clone());
    let entries = SqliteEntryRepository::new(pool.clone());
    let feed_id = feeds
        .upsert_subscription(&NewFeedSubscription {
            site_url: None,
            url: Url::parse("https://example.com/feed.xml").expect("feed url"),
            title: None,
            folder: None,
        })
        .await
        .expect("create feed")
        .id;
    let parsed: Vec<ParsedEntry> =
        titles.iter().enumerate().map(|(index, title)| entry(index, title)).collect();
    entries.upsert_entries(feed_id, &parsed).await.expect("insert entries");
    (pool, entries, feed_id)
}

fn sample_titles() -> Vec<String> {
    TITLES.iter().map(|title| title.to_string()).collect()
}

async fn native_matches(entries: &SqliteEntryRepository, search: &str) -> BTreeSet<String> {
    let query = EntryQuery { search_title: Some(search.to_string()), ..EntryQuery::default() };
    entries
        .list_entries(&query)
        .await
        .expect("search")
        .into_iter()
        .map(|entry| entry.title)
        .collect()
}

fn shared_matches(titles: &[String], search: &str) -> BTreeSet<String> {
    let folded = fold_for_search(search);
    titles.iter().filter(|title| title_contains(title, &folded)).cloned().collect()
}

async fn folded_column(pool: &SqlitePool, dedup_key: &str) -> Option<String> {
    sqlx::query_scalar("SELECT title_search FROM entries WHERE dedup_key = ?1")
        .bind(dedup_key)
        .fetch_one(pool)
        .await
        .expect("read title_search")
}

#[tokio::test]
async fn native_title_search_matches_the_shared_unicode_rule() {
    let titles = sample_titles();
    let (_pool, entries, _) = seeded(&titles).await;
    assert!(shared_matches(&titles, "école").contains("ÉCOLE de Paris"), "the rule folds case");
    for search in searches() {
        let label: String = search.chars().take(20).collect();
        assert_eq!(
            native_matches(&entries, &search).await,
            shared_matches(&titles, &search),
            "{label:?}"
        );
    }
}

/// 迁移 0008 之前写入的条目没有搜索用标题；再次迁移时分批补上（跨越多批），搜索随即按
/// Unicode 匹配。
#[tokio::test]
async fn titles_written_before_the_migration_are_backfilled() {
    let titles: Vec<String> = (0..1_201).map(|index| format!("ÜBER Nummer {index}")).collect();
    let (pool, entries, _) = seeded(&titles).await;
    sqlx::query("UPDATE entries SET title_search = NULL").execute(&pool).await.expect("legacy");
    assert!(
        native_matches(&entries, "über nummer 7").await.is_empty(),
        "unfolded rows: ASCII only"
    );

    migrate(&pool).await.expect("migrate again");

    let rows: Vec<(String, Option<String>)> =
        sqlx::query_as("SELECT title, title_search FROM entries").fetch_all(&pool).await.unwrap();
    assert_eq!(rows.len(), 1_201);
    for (title, folded) in rows {
        assert_eq!(folded.as_deref(), Some(fold_for_search(&title).as_str()), "{title}");
    }
    assert_eq!(
        native_matches(&entries, "über nummer 1200").await,
        shared_matches(&titles, "über nummer 1200")
    );
}

/// 同一条目的标题更新后，搜索用标题跟着更新；只改大小写时也一样（独立评审）。
#[tokio::test]
async fn title_updates_keep_the_folded_column_current() {
    let (pool, entries, feed_id) = seeded(&["École de Paris".to_string()]).await;

    for title in ["Straße und Weg", "STRASSE UND WEG", "Strasse und Weg"] {
        entries.upsert_entries(feed_id, &[entry(0, title)]).await.expect("update title");
        assert_eq!(
            folded_column(&pool, "entry-0").await.as_deref(),
            Some(fold_for_search(title).as_str())
        );
    }
}

/// 升级时还在运行的旧版本程序只改 `title`，不认识搜索列：触发器把过期值清空，搜索退回原标题
/// （ASCII 不区分大小写），下次迁移重新折叠（独立评审）。
#[tokio::test]
async fn an_older_writer_changing_a_title_does_not_leave_a_stale_folded_title() {
    let (pool, entries, _) = seeded(&["Rust Weekly".to_string()]).await;
    sqlx::query("UPDATE entries SET title = 'Python Ünïcode' WHERE dedup_key = 'entry-0'")
        .execute(&pool)
        .await
        .expect("an older writer updates the title");

    assert_eq!(folded_column(&pool, "entry-0").await, None, "the stale value is cleared");
    assert!(native_matches(&entries, "rust").await.is_empty());
    assert_eq!(native_matches(&entries, "PYTHON").await.len(), 1);

    migrate(&pool).await.expect("migrate again");
    assert_eq!(folded_column(&pool, "entry-0").await.as_deref(), Some("python ünïcode"));
    assert_eq!(native_matches(&entries, "ÜNÏCODE").await.len(), 1);
}
