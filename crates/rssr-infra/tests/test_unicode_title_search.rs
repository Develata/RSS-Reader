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

const TITLES: [&str; 7] = [
    "ÉCOLE de Paris",
    "Привет, МИР",
    "ΣΟΦΙΑ και φιλοσοφία",
    "Straße und Weg",
    "Plain ASCII Title",
    "中文标题",
    "100% Pure_Data",
];

const SEARCHES: [&str; 9] =
    ["école", "мир", "σοφια", "STRASSE", "straße", "title", "标题", "0% p", "e_d"];

async fn seeded() -> (SqlitePool, SqliteEntryRepository, i64) {
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
    let parsed: Vec<ParsedEntry> = TITLES
        .iter()
        .enumerate()
        .map(|(index, title)| ParsedEntry {
            external_id: format!("entry-{index}"),
            dedup_key: format!("entry-{index}"),
            url: None,
            title: (*title).to_string(),
            author: None,
            summary: None,
            content_html: None,
            content_text: None,
            published_at: None,
            updated_at_source: None,
        })
        .collect();
    entries.upsert_entries(feed_id, &parsed).await.expect("insert entries");
    (pool, entries, feed_id)
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

fn shared_matches(search: &str) -> BTreeSet<String> {
    let folded = fold_for_search(search);
    TITLES.iter().filter(|title| title_contains(title, &folded)).map(|t| t.to_string()).collect()
}

#[tokio::test]
async fn native_title_search_matches_the_shared_unicode_rule() {
    let (_pool, entries, _) = seeded().await;
    assert!(shared_matches("école").contains("ÉCOLE de Paris"), "the rule folds non-ASCII case");
    for search in SEARCHES {
        assert_eq!(native_matches(&entries, search).await, shared_matches(search), "{search}");
    }
}

/// 迁移 0008 之前写入的条目没有搜索用标题；再次迁移时补上，搜索随即按 Unicode 匹配。
#[tokio::test]
async fn titles_written_before_the_migration_are_backfilled() {
    let (pool, entries, _) = seeded().await;
    sqlx::query("UPDATE entries SET title_search = NULL").execute(&pool).await.expect("legacy");
    assert!(native_matches(&entries, "école").await.is_empty(), "unfolded rows match ASCII only");

    migrate(&pool).await.expect("migrate again");

    let rows: Vec<(String, Option<String>)> =
        sqlx::query_as("SELECT title, title_search FROM entries").fetch_all(&pool).await.unwrap();
    for (title, folded) in rows {
        assert_eq!(folded.as_deref(), Some(fold_for_search(&title).as_str()), "{title}");
    }
    assert_eq!(native_matches(&entries, "école").await, shared_matches("école"));
}
