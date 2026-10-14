//! The JSON shape of the public types (B2-D7): times in RFC3339, counts as numbers, enums
//! with data tagged by `type` in snake_case. Phase C's HTTP backend sends exactly these; the
//! error body is tested with `rssr_api::BackendError`.

use rssr_application::{
    EntryFlagOutcome, ErrorKind, RefreshAllExecutionOutcome, RefreshFailureItem, StartupTarget,
};
use rssr_domain::{
    ArchiveFilter, EntryQuery, FeedSummary, MarkReadOutcome, MarkReadPreview, ReadFilter,
};
use serde_json::json;
use time::macros::datetime;

fn roundtrip<T>(value: &T, expected: serde_json::Value)
where
    T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let encoded = serde_json::to_value(value).unwrap();
    assert_eq!(encoded, expected);
    assert_eq!(&serde_json::from_value::<T>(encoded).unwrap(), value);
}

#[test]
fn times_are_rfc3339_and_counts_are_numbers() {
    let summary = FeedSummary {
        id: 7,
        title: "Feed".into(),
        url: "https://example.com/feed.xml".into(),
        unread_count: 3,
        entry_count: u64::from(u32::MAX) + 1,
        last_fetched_at: Some(datetime!(2026-04-12 08:30:00 UTC)),
        last_success_at: None,
        fetch_error: None,
    };
    roundtrip(
        &summary,
        json!({
            "id": 7,
            "title": "Feed",
            "url": "https://example.com/feed.xml",
            "unread_count": 3,
            "entry_count": 4_294_967_296_u64,
            "last_fetched_at": "2026-04-12T08:30:00Z",
            "last_success_at": null,
            "fetch_error": null,
        }),
    );
}

#[test]
fn enums_with_data_are_tagged_by_type() {
    roundtrip(
        &EntryFlagOutcome::Set { value: true, changed: false },
        json!({ "type": "set", "value": true, "changed": false }),
    );
    roundtrip(&EntryFlagOutcome::Missing, json!({ "type": "missing" }));
    roundtrip(
        &StartupTarget::FeedEntries { feed_id: 7 },
        json!({ "type": "feed_entries", "feed_id": 7 }),
    );
    roundtrip(&StartupTarget::AllEntries, json!({ "type": "all_entries" }));
}

/// The bulk mark-read preview carries its frozen query, whole-second cutoff included.
#[test]
fn the_preview_carries_its_frozen_query() {
    let preview = MarkReadPreview {
        query: EntryQuery {
            read_filter: ReadFilter::UnreadOnly,
            archive_filter: ArchiveFilter::ExcludeArchived {
                cutoff: datetime!(2026-01-12 00:00:00 UTC),
            },
            feed_ids: Some(vec![1, 2]),
            ..EntryQuery::default()
        },
        unread_entry_ids: vec![10, 11],
    };
    let encoded = serde_json::to_value(&preview).unwrap();
    assert_eq!(
        encoded["query"]["archive_filter"],
        json!({ "type": "exclude_archived", "cutoff": "2026-01-12T00:00:00Z" })
    );
    assert_eq!(encoded["query"]["feed_ids"], json!([1, 2]));
    roundtrip(
        &MarkReadOutcome::SelectionChanged { preview: preview.clone() },
        json!({ "type": "selection_changed", "preview": encoded }),
    );
}

#[test]
fn refresh_failures_are_structured() {
    let outcome = RefreshAllExecutionOutcome {
        inserted_count: 2,
        total_count: 3,
        failed_count: 1,
        failures: vec![RefreshFailureItem {
            feed_id: 3,
            url: "https://example.com/three.xml".into(),
            name: Some("Three".into()),
            kind: ErrorKind::Network,
            message: "HTTP 404".into(),
        }],
    };
    roundtrip(
        &outcome,
        json!({
            "inserted_count": 2,
            "total_count": 3,
            "failed_count": 1,
            "failures": [{
                "feed_id": 3,
                "url": "https://example.com/three.xml",
                "name": "Three",
                "kind": "network",
                "message": "HTTP 404",
            }],
        }),
    );
}
