use rssr_domain::{ArchiveFilter, EntryQuery, ReadFilter, StarredFilter};
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
pub fn cases() -> Vec<(EntryQuery, Vec<i64>)> {
    let cutoff = OffsetDateTime::UNIX_EPOCH + time::Duration::days(20454);
    vec![
        (EntryQuery { limit: Some(1), ..Default::default() }, vec![1, 2, 3, 4, 6]),
        (EntryQuery { feed_ids: vec![1], ..Default::default() }, vec![1, 2, 3]),
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
