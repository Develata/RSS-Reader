#![cfg(not(target_arch = "wasm32"))]

//! Lists and counts under every filter combination return exactly the entries the filter
//! describes, in list order, whatever join order the store fixes for the shape (a list over every
//! subscription starts from entries, a scoped list from the planner's choice, a count from the
//! subscriptions) and with `OnlyArchived` written as the negation of `ExcludeArchived`.

#[path = "support/library.rs"]
mod library;

use library::{Library, published};
use rssr_domain::{ArchiveFilter, EntryIndexRepository, EntryQuery, ReadFilter, StarredFilter};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use time::macros::datetime;

const CUTOFF: OffsetDateTime = datetime!(2026-08-12 00:00 UTC);

#[derive(Debug)]
struct Row {
    id: i64,
    feed_id: i64,
    title: String,
    published_at: Option<String>,
    sort_key: String,
    is_read: bool,
    is_starred: bool,
    is_archived: bool,
}

/// Three subscriptions (the last one removed), each with every combination of publication
/// (old, recent, undated) and read / starred / manually archived flags.
async fn library() -> (Library, [i64; 3]) {
    let library = Library::memory().await;
    let mut ids = [0; 3];
    for (slot, name) in ["a", "b", "c"].into_iter().enumerate() {
        let feed = library.add_feed(&format!("https://example.com/{name}.xml")).await;
        ids[slot] = feed.id;
        let mut entries = Vec::new();
        let mut flags = String::new();
        for (index, at) in [Some(datetime!(2020-01-01 00:00 UTC)), Some(CUTOFF), None]
            .into_iter()
            .flat_map(|at| std::iter::repeat_n(at, 8))
            .enumerate()
        {
            let key = format!("{name}-{index}");
            let title = format!("{} {key}", if index % 3 == 0 { "keep" } else { "skip" });
            entries.push(published(&key, &title, at));
            flags.push_str(&format!(
                "UPDATE entries SET is_read = {}, is_starred = {}, is_archived = {} \
                 WHERE dedup_key = '{key}';",
                index & 1,
                (index >> 1) & 1,
                (index >> 2) & 1,
            ));
        }
        library.seed(feed.id, entries).await;
        library.exec(flags).await;
    }
    library.exec(format!("UPDATE feeds SET is_deleted = 1 WHERE id = {}", ids[2])).await;
    (library, ids)
}

async fn rows(library: &Library) -> Vec<Row> {
    library
        .store
        .read(|conn| {
            let mut statement = conn.prepare(
                "SELECT id, feed_id, title, published_at, COALESCE(published_at, created_at),
                        is_read, is_starred, is_archived
                 FROM entries",
            )?;
            Ok(statement
                .query_map([], |row| {
                    Ok(Row {
                        id: row.get(0)?,
                        feed_id: row.get(1)?,
                        title: row.get(2)?,
                        published_at: row.get(3)?,
                        sort_key: row.get(4)?,
                        is_read: row.get(5)?,
                        is_starred: row.get(6)?,
                        is_archived: row.get(7)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?)
        })
        .await
        .expect("rows")
}

/// The filter evaluated on the row in Rust, times compared as times.
fn matches(query: &EntryQuery, row: &Row, removed: i64) -> bool {
    let aged = row
        .published_at
        .as_deref()
        .is_some_and(|at| OffsetDateTime::parse(at, &Rfc3339).expect("stored time") < CUTOFF);
    row.feed_id != removed
        && query.feed_id.is_none_or(|id| id == row.feed_id)
        && query.feed_ids.as_ref().is_none_or(|ids| ids.contains(&row.feed_id))
        && match query.read_filter {
            ReadFilter::All => true,
            ReadFilter::UnreadOnly => !row.is_read,
            ReadFilter::ReadOnly => row.is_read,
        }
        && match query.starred_filter {
            StarredFilter::All => true,
            StarredFilter::StarredOnly => row.is_starred,
            StarredFilter::UnstarredOnly => !row.is_starred,
        }
        && match query.archive_filter {
            ArchiveFilter::All => true,
            ArchiveFilter::ExcludeManual => !row.is_archived,
            ArchiveFilter::OnlyManual => row.is_archived,
            ArchiveFilter::ExcludeArchived { .. } => !row.is_archived && !aged,
            ArchiveFilter::OnlyArchived { .. } => row.is_archived || aged,
        }
        && query.search_title.as_deref().is_none_or(|search| row.title.contains(search))
}

#[tokio::test(flavor = "multi_thread")]
async fn every_filter_combination_lists_and_counts_exactly_its_entries() {
    let (library, [a, _, removed]) = library().await;
    let mut rows = rows(&library).await;
    rows.sort_by(|left, right| right.sort_key.cmp(&left.sort_key).then(right.id.cmp(&left.id)));
    assert_eq!(rows.len(), 72);

    let scopes =
        [(None, None), (Some(a), None), (None, Some(vec![a, removed])), (None, Some(vec![]))];
    let mut checked = 0;
    for read_filter in [ReadFilter::All, ReadFilter::UnreadOnly, ReadFilter::ReadOnly] {
        for starred_filter in
            [StarredFilter::All, StarredFilter::StarredOnly, StarredFilter::UnstarredOnly]
        {
            for archive_filter in [
                ArchiveFilter::All,
                ArchiveFilter::ExcludeManual,
                ArchiveFilter::OnlyManual,
                ArchiveFilter::ExcludeArchived { cutoff: CUTOFF },
                ArchiveFilter::OnlyArchived { cutoff: CUTOFF },
            ] {
                for (feed_id, feed_ids) in scopes.clone() {
                    for search_title in [None, Some("keep".to_string())] {
                        let query = EntryQuery {
                            feed_id,
                            feed_ids: feed_ids.clone(),
                            read_filter,
                            starred_filter,
                            archive_filter,
                            search_title,
                            limit: None,
                        };
                        let expected: Vec<i64> = rows
                            .iter()
                            .filter(|row| matches(&query, row, removed))
                            .map(|row| row.id)
                            .collect();
                        let count = library.entries.count_entries(&query).await.expect("count");
                        assert_eq!(count, expected.len() as u64, "{query:?}");
                        for limit in [None, Some(3)] {
                            let query = EntryQuery { limit, ..query.clone() };
                            let listed: Vec<i64> = library
                                .entries
                                .list_entries(&query)
                                .await
                                .expect("list")
                                .into_iter()
                                .map(|entry| entry.id)
                                .collect();
                            let shown =
                                limit.map_or(expected.len(), |n| expected.len().min(n as usize));
                            assert_eq!(listed, expected[..shown], "{query:?}");
                        }
                        checked += 1;
                    }
                }
            }
        }
    }
    assert_eq!(checked, 3 * 3 * 5 * 4 * 2);
}
