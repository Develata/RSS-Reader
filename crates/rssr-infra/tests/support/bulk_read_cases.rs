use rssr_domain::{
    ArchiveFilter, DomainError, EntryIndexRepository, EntryQuery, MAX_MARK_READ_SELECTION,
    MarkReadOutcome, MarkReadPreview, ReadFilter, StarredFilter,
};
use time::OffsetDateTime;

pub const ROWS: &[(i64, i64, &str, bool, bool, bool)] = &[
    (1, 1, "Rust recent", false, false, false),
    (2, 1, "Rust old", true, true, false),
    (3, 1, "Other undated", false, false, false),
    (4, 2, "Rust other", false, true, false),
    (5, 2, "Rust read", false, true, true),
    (6, 2, "other old", true, false, false),
];
pub fn published(id: i64, old: bool) -> Option<OffsetDateTime> {
    if id == 3 {
        None
    } else {
        Some(OffsetDateTime::UNIX_EPOCH + time::Duration::days(if old { 18262 } else { 20635 }))
    }
}
#[track_caller]
fn assert_refused<T: std::fmt::Debug>(result: rssr_domain::Result<T>, what: &str) {
    let DomainError::InvalidInput(expected) = rssr_domain::mark_read_selection_too_large() else {
        unreachable!()
    };
    match result {
        Err(DomainError::InvalidInput(message)) if message == expected => {}
        other => panic!("{what}: {other:?}"),
    }
}

/// The bound on one bulk mark-read, the same on every store. Expects exactly
/// `MAX_MARK_READ_SELECTION + 1` entries, all unread, `toggle_id` among them.
pub async fn verify_selection_bound(entries: &dyn EntryIndexRepository, toggle_id: i64) {
    let all = EntryQuery::default();
    let unread = EntryQuery { read_filter: ReadFilter::UnreadOnly, ..Default::default() };
    let max = MAX_MARK_READ_SELECTION as u64;

    assert_refused(entries.preview_mark_read(&all).await, "one past the bound");

    entries.set_read(toggle_id, true).await.unwrap();
    let preview = entries.preview_mark_read(&all).await.expect("exactly the bound");
    assert_eq!(preview.unread_entry_ids.len(), MAX_MARK_READ_SELECTION);

    // Grown past the bound before the confirmation: refused, not a whole new selection.
    entries.set_read(toggle_id, false).await.unwrap();
    assert_refused(entries.mark_read_if_unchanged(&preview).await, "grown at confirmation");
    // A small preview against a large selection is refused when the selection is read.
    let forged = MarkReadPreview { query: all.clone(), unread_entry_ids: vec![toggle_id] };
    assert_refused(entries.mark_read_if_unchanged(&forged).await, "small preview");
    assert_eq!(entries.count_entries(&unread).await.unwrap(), max + 1, "nothing was written");

    entries.set_read(toggle_id, true).await.unwrap();
    assert_eq!(
        entries.mark_read_if_unchanged(&preview).await.unwrap(),
        MarkReadOutcome::Applied { changed_count: max }
    );
    // Read entries are not counted against the bound.
    assert!(entries.preview_mark_read(&all).await.unwrap().unread_entry_ids.is_empty());
    let read_only = EntryQuery { read_filter: ReadFilter::ReadOnly, ..Default::default() };
    assert!(entries.preview_mark_read(&read_only).await.unwrap().unread_entry_ids.is_empty());
}

pub fn cases() -> Vec<(EntryQuery, Vec<i64>)> {
    let cutoff = OffsetDateTime::UNIX_EPOCH + time::Duration::days(20454);
    vec![
        (EntryQuery { limit: Some(1), ..Default::default() }, vec![1, 2, 3, 4, 6]),
        (EntryQuery { feed_ids: Some(vec![1]), ..Default::default() }, vec![1, 2, 3]),
        // Chosen sources that are all gone match nothing, never everything.
        (EntryQuery { feed_ids: Some(Vec::new()), ..Default::default() }, vec![]),
        (EntryQuery { search_title: Some("Rust".into()), ..Default::default() }, vec![1, 2, 4]),
        (
            EntryQuery {
                archive_filter: ArchiveFilter::ExcludeArchived { cutoff },
                ..Default::default()
            },
            vec![1, 3, 4],
        ),
        (
            EntryQuery { starred_filter: StarredFilter::StarredOnly, ..Default::default() },
            vec![2, 4],
        ),
        (
            EntryQuery {
                feed_id: Some(1),
                search_title: Some("Rust".into()),
                starred_filter: StarredFilter::StarredOnly,
                archive_filter: ArchiveFilter::OnlyArchived { cutoff },
                ..Default::default()
            },
            vec![2],
        ),
        (EntryQuery { read_filter: ReadFilter::ReadOnly, ..Default::default() }, vec![]),
        (
            EntryQuery {
                read_filter: ReadFilter::UnreadOnly,
                starred_filter: StarredFilter::UnstarredOnly,
                ..Default::default()
            },
            vec![1, 3, 6],
        ),
    ]
}
