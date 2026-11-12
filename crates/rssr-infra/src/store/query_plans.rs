//! Query plans the library relies on, checked against the bundled SQLite.
//!
//! Libraries never run `ANALYZE`, so the planner works from default estimates and an empty
//! library is planned exactly like a large one. Each test pins a plan whose loss turns a bounded
//! read back into a read of every entry row (measured on 50k- and 250k-entry libraries).

use rssr_domain::{ArchiveFilter, EntryQuery, ReadFilter, StarredFilter};
use rusqlite::{Connection, params_from_iter, types::Value};
use time::macros::datetime;

use super::connection::{Role, configure};
use super::entries::{count_statement, list_statement};
use super::feeds::summaries_sql;
use super::migrate::migrate;

const CUTOFF: time::OffsetDateTime = datetime!(2026-08-12 00:00 UTC);

fn library() -> Connection {
    let mut connection = Connection::open_in_memory().expect("open");
    configure(&connection, Role::Writer).expect("configure");
    migrate(&mut connection, "2026-11-12T00:00:00Z").expect("migrate");
    connection
}

fn plan(connection: &Connection, sql: &str, params: &[Value]) -> String {
    let mut statement = connection.prepare(&format!("EXPLAIN QUERY PLAN {sql}")).expect("plan");
    statement
        .query_map(params_from_iter(params), |row| row.get::<_, String>(3))
        .expect("plan rows")
        .collect::<Result<Vec<_>, _>>()
        .expect("plan detail")
        .join(" | ")
}

const READS: [ReadFilter; 3] = [ReadFilter::All, ReadFilter::UnreadOnly, ReadFilter::ReadOnly];
const STARS: [StarredFilter; 3] =
    [StarredFilter::All, StarredFilter::StarredOnly, StarredFilter::UnstarredOnly];
const ARCHIVES: [ArchiveFilter; 5] = [
    ArchiveFilter::All,
    ArchiveFilter::ExcludeManual,
    ArchiveFilter::OnlyManual,
    ArchiveFilter::ExcludeArchived { cutoff: CUTOFF },
    ArchiveFilter::OnlyArchived { cutoff: CUTOFF },
];

/// Every read / starred / archive / scope combination without a title search.
fn queries() -> impl Iterator<Item = EntryQuery> {
    let scopes = [(None, None), (Some(17), None), (None, Some(vec![3, 10, 17, 24]))];
    READS.into_iter().flat_map(move |read_filter| {
        let scopes = scopes.clone();
        STARS.into_iter().flat_map(move |starred_filter| {
            let scopes = scopes.clone();
            ARCHIVES.into_iter().flat_map(move |archive_filter| {
                scopes.clone().into_iter().map(move |(feed_id, feed_ids)| EntryQuery {
                    feed_id,
                    feed_ids,
                    read_filter,
                    starred_filter,
                    archive_filter,
                    search_title: None,
                    limit: Some(2001),
                })
            })
        })
    })
}

/// The default pages (every subscription; all, manual or automatic archive hidden) take one
/// page from the sort-key index instead of sorting every entry.
#[test]
fn lists_over_every_subscription_walk_the_sort_key_index() {
    let connection = library();
    for archive_filter in [
        ArchiveFilter::All,
        ArchiveFilter::ExcludeManual,
        ArchiveFilter::ExcludeArchived { cutoff: CUTOFF },
    ] {
        let query = EntryQuery { archive_filter, limit: Some(2001), ..EntryQuery::default() };
        let sql = list_statement(&query).expect("list");
        let plan = plan(&connection, &sql.sql, &sql.params);
        assert!(plan.starts_with("SCAN entries USING INDEX idx_entries_sort_key"), "{plan}");
        assert!(!plan.contains("TEMP B-TREE"), "{plan}");
    }
}

/// Every count (the archived count beside each list, the overview counts) reads an index of
/// each subscription's entries, never the entry rows.
#[test]
fn counts_read_only_an_index() {
    let connection = library();
    for query in queries() {
        let sql = count_statement(&query).expect("count");
        let plan = plan(&connection, &sql.sql, &sql.params);
        assert!(plan.contains("SEARCH entries USING COVERING INDEX"), "{query:?}: {plan}");
    }
}

#[test]
fn feed_summaries_read_only_the_flag_index() {
    let connection = library();
    let plan = plan(&connection, &summaries_sql(), &[]);
    assert!(plan.contains("SEARCH entries USING COVERING INDEX idx_entries_feed_flags"), "{plan}");
}

/// `OnlyArchived` is written as a negation; as an `OR` the planner turned some read / starred /
/// multi-subscription lists into a union of index scans several times slower.
#[test]
fn only_archived_is_never_split_into_an_index_union() {
    let connection = library();
    for query in
        queries().filter(|query| matches!(query.archive_filter, ArchiveFilter::OnlyArchived { .. }))
    {
        for sql in [list_statement(&query), count_statement(&query)] {
            let sql = sql.expect("statement");
            let plan = plan(&connection, &sql.sql, &sql.params);
            assert!(!plan.contains("MULTI-INDEX OR"), "{query:?}: {plan}");
        }
    }
}
