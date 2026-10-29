#![cfg(any(not(target_arch = "wasm32"), feature = "memory-store"))]

//! Shared port contracts against the same v0.2 SQL repositories on native and browser SQLite.

#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

#[path = "support/archive_boundary_cases.rs"]
mod archive_boundary_cases;
#[path = "support/bulk_read_cases.rs"]
mod bulk_read_cases;
#[path = "support/feed_title_cases.rs"]
mod feed_title_cases;
#[path = "support/library.rs"]
mod library;
#[path = "support/listed_title_cases.rs"]
mod listed_title_cases;
#[path = "support/refresh_content_cases.rs"]
mod refresh_content_cases;
#[path = "support/refresh_count_cases.rs"]
mod refresh_count_cases;

use library::Library;
use rssr_domain::{EntryIndexRepository, EntryQuery, FeedRepository, MarkReadOutcome};

#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
async fn offline_composition_refuses_network_even_through_direct_use_cases() {
    use rssr_application::{ErrorKind, classify};
    let library = Library::memory().await;
    let feed = library.add_feed("https://example.com/stored.xml").await;
    let cases =
        rssr_infra::composition::compose_offline_library_use_cases(library.store.clone()).use_cases;
    let error = cases
        .subscription_workflow
        .prepare_subscription("https://example.com/new.xml")
        .await
        .unwrap_err();
    assert_eq!(classify(&error).kind, ErrorKind::Network);
    let outcome = cases.refresh_service.refresh_feed(feed.id).await.unwrap();
    assert!(matches!(
        outcome.result,
        rssr_application::RefreshFeedResult::Failed { kind: ErrorKind::Network, .. }
    ));
    assert_eq!(library.feeds.list_feeds().await.unwrap().len(), 1);
}

#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
async fn concurrent_handles_preserve_other_entries_and_other_flag_bits() {
    let library = Library::memory().await;
    let other = Library::over(library.store.clone());
    let (first, second) = futures_util::join!(
        library.add_feed("https://example.com/first"),
        other.add_feed("https://example.com/second"),
    );
    assert_ne!(first.id, second.id);
    let (one, two) = futures_util::join!(
        library.seed(first.id, vec![library::entry("first")]),
        other.seed(second.id, vec![library::entry("second")]),
    );
    assert_eq!((one, two), (1, 1));
    let entries = library.entries.list_entries(&EntryQuery::default()).await.unwrap();
    let (one, two) = (entries[0].id, entries[1].id);
    let (read, star, other_read) = futures_util::join!(
        library.entries.set_read(one, true),
        other.entries.set_starred(one, true),
        other.entries.set_read(two, true),
    );
    read.unwrap();
    star.unwrap();
    other_read.unwrap();
    let stored = library.entries.get_entry_record(one).await.unwrap().unwrap();
    assert!(stored.is_read && stored.is_starred);
    assert!(library.entries.get_entry_record(two).await.unwrap().unwrap().is_read);
    assert_eq!(library.entries.count_entries(&EntryQuery::default()).await.unwrap(), 2);
}

#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
async fn listing_and_flag_writes_do_not_need_article_bodies() {
    let library = Library::memory().await;
    let feed = library.add_feed("https://example.com/body-independent").await;
    library.seed(feed.id, vec![library::entry("first")]).await;
    library.exec("DROP TABLE entry_contents").await;
    assert_eq!(library.entries.list_entries(&EntryQuery::default()).await.unwrap().len(), 1);
    library.entries.set_read(1, true).await.unwrap();
    library.entries.set_starred(1, true).await.unwrap();
    assert_eq!(library.feeds.list_summaries().await.unwrap()[0].unread_count, 0);
}

#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
async fn committed_refreshes_survive_batch_boundaries() {
    use rssr_application::RefreshStorePort;
    let library = Library::memory().await;
    let feed = library.add_feed("https://example.com/batch").await;
    library.refresh.begin_batch().await.unwrap();
    library.seed(feed.id, vec![library::entry("a")]).await;
    assert_eq!(library.entries.count_entries(&EntryQuery::default()).await.unwrap(), 1);
    library.refresh.begin_batch().await.unwrap();
    library.refresh.abort_batch();
    assert_eq!(library.entries.count_entries(&EntryQuery::default()).await.unwrap(), 1);
    library.seed(feed.id, vec![library::entry("b")]).await;
    library.refresh.end_batch().await.unwrap();
    library.refresh.abort_batch();
    library.refresh.begin_batch().await.unwrap();
    library.refresh.end_batch().await.unwrap();
    assert_eq!(library.entries.count_entries(&EntryQuery::default()).await.unwrap(), 2);
}

#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
async fn metadata_only_refreshes_do_not_rewrite_flags_or_bodies() {
    use rssr_application::{RefreshCommit, RefreshFailure, RefreshHttpMetadata, RefreshStorePort};
    let library = Library::memory().await;
    let feed = library.add_feed("https://example.com/metadata").await;
    library
        .seed(
            feed.id,
            vec![rssr_application::ParsedEntryData {
                content_html: Some("<p>body</p>".into()),
                ..library::entry("a")
            }],
        )
        .await;
    library.entries.set_read(1, true).await.unwrap();
    library.entries.set_starred(1, true).await.unwrap();
    library.exec("CREATE TRIGGER reject_body BEFORE UPDATE ON entry_contents BEGIN SELECT RAISE(FAIL,'body rewritten'); END;
        CREATE TRIGGER reject_flags BEFORE UPDATE OF is_read, is_starred, read_at, starred_at ON entries BEGIN SELECT RAISE(FAIL,'flags rewritten'); END;").await;
    for commit in [
        RefreshCommit::NotModified {
            metadata: RefreshHttpMetadata { etag: Some("unchanged".into()), last_modified: None },
        },
        RefreshCommit::Failed { failure: RefreshFailure::fetch("offline", None) },
    ] {
        let target = library.refresh.get_target(feed.id).await.unwrap().unwrap();
        library.refresh.commit(&target, commit).await.unwrap();
    }
    let entry = library.entries.get_entry(1).await.unwrap().unwrap();
    assert!(entry.is_read && entry.is_starred);
    assert_eq!(entry.content_html.as_deref(), Some("<p>body</p>"));
}

#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
async fn chosen_names_stay_apart_from_source_titles() {
    let library = Library::memory().await;
    feed_title_cases::verify_custom_titles(&feed_title_cases::Ports {
        feeds: &library.feeds,
        entries: &library.entries,
        activation: &library.mutations,
        removal: &library.mutations,
        refresh: &library.refresh,
        config: &library.mutations,
        queries: &library.queries,
    })
    .await;
}

#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
async fn fractional_published_times_are_archived_by_their_value() {
    let library = Library::memory().await;
    let feed = library.add_feed("https://example.com/archive-boundary.xml").await;
    archive_boundary_cases::verify_fractional_published_times(
        &library.refresh,
        &library.entries,
        feed.id,
    )
    .await;
}

#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
async fn refresh_rewrites_only_changed_bodies() {
    let library = Library::memory().await;
    let feed = library.add_feed("https://example.com/content.xml").await;
    refresh_content_cases::verify_content_changes(
        &library.refresh,
        &library.entries,
        &library.entries,
        feed.id,
    )
    .await;

    // Observe database writes directly: an unchanged source must not rewrite its body.
    library
        .exec(
            "CREATE TRIGGER reject_redundant_update BEFORE UPDATE ON entry_contents
             BEGIN SELECT RAISE(FAIL, 'redundant content write'); END",
        )
        .await;
    let target = rssr_application::RefreshStorePort::get_target(&library.refresh, feed.id)
        .await
        .unwrap()
        .unwrap();
    let stored = library.entries.get_entry(1).await.unwrap().unwrap();
    let same = rssr_application::ParsedEntryData {
        content_html: stored.content_html,
        content_text: stored.content_text,
        url: stored.url,
        title: stored.title,
        ..library::entry(&stored.dedup_key)
    };
    rssr_application::RefreshStorePort::commit(
        &library.refresh,
        &target,
        library::updated(vec![same]),
    )
    .await
    .expect("an unchanged body is not rewritten");
}

#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
async fn counts_only_real_inserts_including_same_batch_duplicates() {
    let library = Library::memory().await;
    let feed = library.add_feed("https://example.com/count").await;
    refresh_count_cases::verify_counts(&library.refresh, feed.id).await;
    assert_eq!(library.feeds.list_summaries().await.unwrap()[0].entry_count, 3);
}

/// Titles in lists are cut by the shared rule, in the query; the reader keeps the whole title.
#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
async fn titles_in_lists_are_bounded() {
    use listed_title_cases::{bound_entry_title, long_entry_title, long_feed_title, nul_title};
    use rssr_domain::listed_title;

    let library = Library::memory().await;
    let feed = library.add_feed("https://example.com/long").await;
    library
        .seed(
            feed.id,
            vec![
                library::published("long", &long_entry_title(), None),
                library::published("bound", &bound_entry_title(), None),
                library::published("nul", &nul_title(), None),
            ],
        )
        .await;
    library.exec(format!("UPDATE feeds SET title = '{}'", long_feed_title())).await;
    let nul_feed = library.add_feed("https://example.com/nul").await;
    library
        .exec(format!(
            "UPDATE feeds SET title = 'a' || char(0) || '{}' WHERE id = {}",
            "x".repeat(4 * rssr_domain::MAX_LISTED_TITLE_CHARS),
            nul_feed.id
        ))
        .await;
    let summaries = library.feeds.list_summaries().await.unwrap();
    let nul_summary = summaries.iter().find(|summary| summary.id == nul_feed.id).unwrap();
    assert_eq!(nul_summary.title, listed_title(&nul_title()), "a NUL does not stop the cut");
    // Text that is not UTF-8 (written outside the program) is shown with replacements.
    library
        .exec(format!(
            "UPDATE feeds SET title = CAST(x'61ff62' AS TEXT) WHERE id = {}",
            nul_feed.id
        ))
        .await;
    let summaries = library.feeds.list_summaries().await.unwrap();
    let invalid = summaries.iter().find(|summary| summary.id == nul_feed.id).unwrap();
    assert_eq!(invalid.title, "a\u{fffd}b");

    let listed = library.entries.list_entries(&EntryQuery::default()).await.unwrap();
    let mut titles: Vec<_> = listed.iter().map(|entry| entry.title.clone()).collect();
    titles.sort();
    let mut expected =
        vec![listed_title(&long_entry_title()), bound_entry_title(), listed_title(&nul_title())];
    expected.sort();
    assert_eq!(titles, expected);
    assert!(listed.iter().all(|entry| entry.feed_title == listed_title(&long_feed_title())));
    let long_summary = summaries.iter().find(|summary| summary.id == feed.id).unwrap();
    assert_eq!(long_summary.title, listed_title(&long_feed_title()));
    let long =
        listed.iter().find(|entry| entry.title == listed_title(&long_entry_title())).unwrap();
    let record = library.entries.get_entry_record(long.id).await.unwrap().unwrap();
    assert_eq!(record.title, long_entry_title(), "the reader keeps the whole title");
}

#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
async fn bulk_read_refuses_a_selection_over_its_bound() {
    let library = Library::memory().await;
    let feed = library.add_feed("https://example.com/many").await;
    let count = rssr_domain::MAX_MARK_READ_SELECTION + 1;
    let parsed = (0..count).map(|key| library::published(&key.to_string(), "Entry", None));
    assert_eq!(library.seed(feed.id, parsed.collect()).await, count as u64);
    let toggle = library.scalar("SELECT MIN(id) FROM entries").await;
    bulk_read_cases::verify_selection_bound(&library.entries, toggle).await;
}

#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
async fn bulk_read_scope_confirmation_atomic_failure_and_zero() {
    let library = Library::memory().await;
    let entries = &library.entries;
    for feed_id in [1, 2] {
        library.add_feed(&format!("https://example.com/{feed_id}")).await;
    }
    for &(id, feed_id, title, old, starred, read) in bulk_read_cases::ROWS {
        library
            .seed(
                feed_id,
                vec![library::published(
                    &id.to_string(),
                    title,
                    bulk_read_cases::published(id, old),
                )],
            )
            .await;
        entries.set_starred(id, starred).await.unwrap();
        entries.set_read(id, read).await.unwrap();
    }
    for (query, expected) in bulk_read_cases::cases() {
        let preview = entries.preview_mark_read(&query).await.unwrap();
        assert_eq!(preview.unread_entry_ids, expected, "{query:?}");
        assert_eq!(preview.query.limit, None);
    }
    let original = entries.preview_mark_read(&EntryQuery::default()).await.unwrap();
    entries.set_read(1, true).await.unwrap();
    entries.set_read(5, false).await.unwrap();
    assert!(matches!(
        entries.mark_read_if_unchanged(&original).await.unwrap(),
        MarkReadOutcome::SelectionChanged { .. }
    ));
    assert!(!entries.get_entry_record(2).await.unwrap().unwrap().is_read);
    let fresh = entries.preview_mark_read(&EntryQuery::default()).await.unwrap();
    library
        .exec(
            "CREATE TRIGGER reject_batch BEFORE UPDATE OF is_read ON entries WHEN NEW.id = 4
             BEGIN SELECT RAISE(ABORT, 'test failure'); END",
        )
        .await;
    assert!(entries.mark_read_if_unchanged(&fresh).await.is_err());
    assert_eq!(entries.preview_mark_read(&EntryQuery::default()).await.unwrap(), fresh);
    library.exec("DROP TRIGGER reject_batch").await;
    assert_eq!(
        entries.mark_read_if_unchanged(&fresh).await.unwrap(),
        MarkReadOutcome::Applied { changed_count: 5 }
    );
    for id in fresh.unread_entry_ids {
        assert!(entries.get_entry_record(id).await.unwrap().unwrap().read_at.is_some());
    }
    let empty = entries.preview_mark_read(&EntryQuery::default()).await.unwrap();
    assert_eq!(
        entries.mark_read_if_unchanged(&empty).await.unwrap(),
        MarkReadOutcome::Applied { changed_count: 0 }
    );
    assert!(library.feeds.list_summaries().await.unwrap().iter().all(|f| f.unread_count == 0));
}

/// A scope larger than SQLite's parameter limit keeps working (one JSON parameter).
#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
async fn entry_query_accepts_more_feed_ids_than_the_parameter_limit() {
    let library = Library::memory().await;
    let feed = library.add_feed("https://example.com/selected").await;
    library.seed(feed.id, vec![library::entry("selected-entry")]).await;
    let entries = &library.entries;
    let query = EntryQuery { feed_ids: Some((1_i64..=40_000).collect()), ..EntryQuery::default() };
    assert_eq!(entries.count_entries(&query).await.unwrap(), 1);
    assert_eq!(entries.list_entries(&query).await.unwrap().len(), 1);
    let preview = entries.preview_mark_read(&query).await.unwrap();
    assert_eq!(preview.unread_entry_ids.len(), 1);
    assert_eq!(
        entries.mark_read_if_unchanged(&preview).await.unwrap(),
        MarkReadOutcome::Applied { changed_count: 1 }
    );
}

#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg(not(target_arch = "wasm32"))]
async fn bulk_read_large_dataset_measurement() {
    let library = Library::memory().await;
    library
        .exec(
            "INSERT INTO feeds(id, url, created_at, updated_at)
             VALUES (1, 'https://example.com', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z');
             WITH RECURSIVE seq(x) AS (VALUES(1) UNION ALL SELECT x + 1 FROM seq WHERE x < 50000)
             INSERT INTO entries(feed_id, external_id, dedup_key, title, title_search,
                                 first_seen_at, created_at, updated_at)
             SELECT 1, CAST(x AS TEXT), CAST(x AS TEXT), 'Bulk measurement', 'bulk measurement',
                    '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z'
             FROM seq",
        )
        .await;
    let entries = &library.entries;
    // Over the bound: refused after reading one id past it.
    let started = std::time::Instant::now();
    assert!(entries.preview_mark_read(&EntryQuery::default()).await.is_err());
    let refuse_ms = started.elapsed().as_millis();
    // At the bound: the largest selection a bulk mark-read covers.
    let max = rssr_domain::MAX_MARK_READ_SELECTION;
    library.exec(format!("UPDATE entries SET is_read = 1 WHERE id > {max}")).await;
    let started = std::time::Instant::now();
    let preview = entries.preview_mark_read(&EntryQuery::default()).await.unwrap();
    let preview_ms = started.elapsed().as_millis();
    let started = std::time::Instant::now();
    assert_eq!(
        entries.mark_read_if_unchanged(&preview).await.unwrap(),
        MarkReadOutcome::Applied { changed_count: max as u64 }
    );
    eprintln!(
        "bulk_read rows=50000 refuse_ms={refuse_ms} selected={max} preview_ms={preview_ms} apply_ms={}",
        started.elapsed().as_millis()
    );
}
