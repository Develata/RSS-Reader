#![cfg(not(target_arch = "wasm32"))]

//! 标题搜索在原生端按 Unicode 小写比较，与浏览器端（同一个 `rssr_domain::title_contains`）
//! 匹配到同样的文章（审计 D6）。v0.2 资料库版，原 `test_unicode_title_search`。
//!
//! 不再移植的旧用例（均因 v0.2 基线 `entries.title_search` 为 NOT NULL，没有历史库、回填、
//! 过期值触发器与 ASCII 退路）：迁移 0008 之前写入的条目回填、旧版本程序只改 `title` 的触发器、
//! 从迁移 0007 升级并回填、回填中途失败后续补。它们背后的不变量「每个标题行的搜索列都等于
//! `fold_for_search(title)`」由 `every_written_title_has_its_folded_search_column` 覆盖。

#[path = "support/library.rs"]
mod library;

use std::collections::BTreeSet;

use library::Library;
use rssr_application::ParsedEntryData;
use rssr_domain::{EntryIndexRepository, EntryQuery, fold_for_search, title_contains};

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

fn entry(index: usize, title: &str) -> ParsedEntryData {
    ParsedEntryData {
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

async fn seeded(titles: &[String]) -> (Library, i64) {
    let library = Library::memory().await;
    let feed_id = library.add_feed("https://example.com/feed.xml").await.id;
    let parsed: Vec<ParsedEntryData> =
        titles.iter().enumerate().map(|(index, title)| entry(index, title)).collect();
    library.seed(feed_id, parsed).await;
    (library, feed_id)
}

fn sample_titles() -> Vec<String> {
    TITLES.iter().map(|title| title.to_string()).collect()
}

async fn native_matches(library: &Library, search: &str) -> BTreeSet<String> {
    let query = EntryQuery { search_title: Some(search.to_string()), ..EntryQuery::default() };
    library
        .entries
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

async fn folded_column(library: &Library, dedup_key: &str) -> Option<String> {
    library.text(format!("SELECT title_search FROM entries WHERE dedup_key = '{dedup_key}'")).await
}

#[tokio::test]
async fn native_title_search_matches_the_shared_unicode_rule() {
    let titles = sample_titles();
    let (library, _) = seeded(&titles).await;
    assert!(shared_matches(&titles, "école").contains("ÉCOLE de Paris"), "the rule folds case");
    for search in searches() {
        let label: String = search.chars().take(20).collect();
        assert_eq!(
            native_matches(&library, &search).await,
            shared_matches(&titles, &search),
            "{label:?}"
        );
    }
}

/// Replaces the backfill tests: with no legacy rows, the invariant that remains is that every
/// written row (here across 1_201 entries, the span of the old backfill batches) carries
/// `fold_for_search(title)` and is found by Unicode search.
#[tokio::test]
async fn every_written_title_has_its_folded_search_column() {
    let titles: Vec<String> = (0..1_201).map(|index| format!("ÜBER Nummer {index}")).collect();
    let (library, _) = seeded(&titles).await;

    let rows: Vec<(String, String)> = library
        .store
        .read(|conn| {
            let mut statement = conn.prepare("SELECT title, title_search FROM entries")?;
            let rows = statement
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await
        .unwrap();
    assert_eq!(rows.len(), 1_201);
    for (title, folded) in rows {
        assert_eq!(folded, fold_for_search(&title), "{title}");
    }
    assert_eq!(
        native_matches(&library, "über nummer 1200").await,
        shared_matches(&titles, "über nummer 1200")
    );
}

/// 同一条目的标题更新后，搜索用标题跟着更新；只改大小写时也一样（独立评审）。
#[tokio::test]
async fn title_updates_keep_the_folded_column_current() {
    let (library, feed_id) = seeded(&["École de Paris".to_string()]).await;

    for title in ["Straße und Weg", "STRASSE UND WEG", "Strasse und Weg"] {
        library.seed(feed_id, vec![entry(0, title)]).await;
        assert_eq!(
            folded_column(&library, "entry-0").await.as_deref(),
            Some(fold_for_search(title).as_str())
        );
    }
}
