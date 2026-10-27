//! Published times with a fraction of a second, committed through a store's refresh port, are
//! archived as their time values say against a whole-second cutoff: the list, the count and the
//! bulk-read set agree, on the SQLite store and on the browser store (B2-D7). In RFC 3339 text
//! `…10:00:00.5Z` sorts before `…10:00:00Z`, so the stores keep published times in whole seconds
//! (`rssr_domain::stored_published_at`); for a whole-second cutoff that changes no comparison.

use rssr_application::{
    FeedRefreshUpdate, ParsedEntryData, ParsedFeedUpdate, RefreshCommit, RefreshStorePort,
};
use rssr_domain::{ArchiveFilter, EntryIndexRepository, EntryQuery};
use time::{Duration, OffsetDateTime};

fn entry(title: &str, published_at: OffsetDateTime) -> ParsedEntryData {
    ParsedEntryData {
        external_id: title.into(),
        dedup_key: title.into(),
        url: None,
        title: title.into(),
        author: None,
        summary: None,
        content_html: None,
        content_text: None,
        published_at: Some(published_at),
        updated_at_source: None,
    }
}

async fn listed(entries: &dyn EntryIndexRepository, query: &EntryQuery) -> Vec<(i64, String)> {
    let mut listed = entries
        .list_entries(query)
        .await
        .expect("list entries")
        .into_iter()
        .map(|entry| {
            let published_at = entry.published_at.expect("every entry has a published time");
            assert_eq!(published_at.nanosecond(), 0, "{} is stored in whole seconds", entry.title);
            (entry.id, entry.title)
        })
        .collect::<Vec<_>>();
    listed.sort_by(|left, right| left.1.cmp(&right.1));
    listed
}

pub async fn verify_fractional_published_times(
    store: &dyn RefreshStorePort,
    entries: &dyn EntryIndexRepository,
    feed_id: i64,
) {
    let cutoff = OffsetDateTime::from_unix_timestamp(1_772_359_200).expect("a whole second");
    let half = Duration::milliseconds(500);
    let target = store.get_target(feed_id).await.expect("target").expect("active subscription");
    store
        .commit(
            &target,
            RefreshCommit::Updated {
                update: FeedRefreshUpdate {
                    metadata: Default::default(),
                    feed: ParsedFeedUpdate {
                        title: None,
                        site_url: None,
                        description: None,
                        entries: vec![
                            entry("a: half a second before", cutoff - half),
                            entry("b: at the cutoff", cutoff),
                            entry("c: half a second after", cutoff + half),
                        ],
                    },
                },
            },
        )
        .await
        .expect("commit the entries");

    let kept = EntryQuery {
        archive_filter: ArchiveFilter::ExcludeArchived { cutoff },
        ..EntryQuery::default()
    };
    let archived = EntryQuery {
        archive_filter: ArchiveFilter::OnlyArchived { cutoff },
        ..EntryQuery::default()
    };

    let kept_entries = listed(entries, &kept).await;
    let titles = |listed: &[(i64, String)]| {
        listed.iter().map(|(_, title)| title.clone()).collect::<Vec<_>>()
    };
    assert_eq!(titles(&kept_entries), ["b: at the cutoff", "c: half a second after"]);
    assert_eq!(titles(&listed(entries, &archived).await), ["a: half a second before"]);
    assert_eq!(entries.count_entries(&kept).await.expect("count kept"), 2);
    assert_eq!(entries.count_entries(&archived).await.expect("count archived"), 1);

    let mut to_mark = entries.preview_mark_read(&kept).await.expect("preview").unread_entry_ids;
    to_mark.sort_unstable();
    let mut kept_ids = kept_entries.iter().map(|(id, _)| *id).collect::<Vec<_>>();
    kept_ids.sort_unstable();
    assert_eq!(to_mark, kept_ids, "marking the list read marks exactly the listed entries");
}
