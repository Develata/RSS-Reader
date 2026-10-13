#![cfg(not(target_arch = "wasm32"))]

//! Parsed feeds flowing into the v0.2 library store (ported from `test_feed_refresh_flow.rs`).
//!
//! v0.1.x wrote entries and feed metadata with separate repository calls; the library takes
//! them in one refresh commit, so the tests go through `RefreshStorePort::commit`.

#[path = "support/library.rs"]
mod library;

use library::Library;
use rssr_application::{
    FeedRefreshUpdate, ParsedEntryData, ParsedFeedUpdate, RefreshCommit, RefreshStorePort,
};
use rssr_domain::{EntryIndexRepository, EntryQuery, FeedRepository, NewFeedSubscription};
use rssr_infra::{parser::FeedParser, store::LocalizedBody};
use std::sync::Arc;
use url::Url;

const SAMPLE_FEED: &str = r#"
<rss version="2.0">
  <channel>
    <title>Example Feed</title>
    <link>https://example.com</link>
    <description>Example description</description>
    <item>
      <guid>entry-1</guid>
      <title>Hello World</title>
      <link>https://example.com/hello</link>
      <description>First summary</description>
      <pubDate>Tue, 25 Mar 2026 10:00:00 GMT</pubDate>
    </item>
    <item>
      <guid>entry-1</guid>
      <title>Hello World Updated</title>
      <link>https://example.com/hello</link>
      <description>Updated summary</description>
      <pubDate>Tue, 25 Mar 2026 10:00:00 GMT</pubDate>
    </item>
  </channel>
</rss>
"#;

fn revision(key: &str, url: &str, title: &str, html: &str) -> ParsedEntryData {
    ParsedEntryData {
        external_id: key.to_string(),
        dedup_key: key.to_string(),
        url: Some(Url::parse(url).unwrap()),
        title: title.to_string(),
        author: None,
        summary: Some("summary".to_string()),
        content_html: Some(html.to_string()),
        content_text: Some("summary".to_string()),
        published_at: None,
        updated_at_source: None,
    }
}

fn subscription(url: &str, title: &str) -> NewFeedSubscription {
    NewFeedSubscription {
        site_url: None,
        url: Url::parse(url).expect("valid url"),
        title: Some(title.to_string()),
        source_title: None,
        folder: None,
    }
}

async fn write_localized(
    library: &Library,
    feed_id: i64,
    generation: i64,
    dedup_key: &str,
    expected_hash: Option<&str>,
    base_html: &str,
    localized_html: &str,
) -> bool {
    library
        .entries
        .update_localized_body(
            feed_id,
            generation,
            LocalizedBody {
                dedup_key: dedup_key.to_string(),
                expected_content_hash: expected_hash.map(str::to_owned),
                base_html: Arc::from(base_html),
                localized_html: localized_html.to_string(),
            },
        )
        .await
        .expect("attempt localized writeback")
}

#[tokio::test]
async fn refresh_flow_stores_feed_and_deduplicated_entries() {
    let library = Library::memory().await;
    let parser = FeedParser::new();

    let feed = library
        .feeds
        .upsert_subscription(&subscription("https://example.com/feed.xml", "Example Feed"))
        .await
        .expect("create feed");

    let parsed = parser.parse(SAMPLE_FEED).expect("parse sample feed");
    // Entries and feed metadata are one commit in the library (v0.1.x: two repository calls).
    let target = library.refresh.get_target(feed.id).await.unwrap().unwrap();
    let outcome = library
        .refresh
        .commit(
            &target,
            RefreshCommit::Updated {
                update: FeedRefreshUpdate {
                    metadata: Default::default(),
                    feed: ParsedFeedUpdate {
                        title: parsed.title.clone(),
                        site_url: parsed.site_url.clone(),
                        description: parsed.description.clone(),
                        entries: parsed
                            .entries
                            .iter()
                            .map(|entry| ParsedEntryData {
                                external_id: entry.external_id.clone(),
                                dedup_key: entry.dedup_key.clone(),
                                url: entry.url.clone(),
                                title: entry.title.clone(),
                                author: entry.author.clone(),
                                summary: entry.summary.clone(),
                                content_html: entry.content_html.clone(),
                                content_text: entry.content_text.clone(),
                                published_at: entry.published_at,
                                updated_at_source: entry.updated_at_source,
                            })
                            .collect(),
                    },
                },
            },
        )
        .await
        .expect("commit parsed feed");

    // v0.1.x `upsert_entries` returned the number of entries it was given (2); the commit
    // reports real insertions, so the duplicated GUID counts once. The stored rows below are
    // the deduplication contract.
    assert_eq!(parsed.entries.len(), 2);
    assert_eq!(outcome.inserted_count, 1);

    let feeds = library.feeds.list_summaries().await.expect("list feed summaries");
    assert_eq!(feeds.len(), 1);
    assert_eq!(feeds[0].unread_count, 1);
    assert_eq!(feeds[0].entry_count, 1);

    let stored_feed =
        library.feeds.get_feed(feed.id).await.expect("read stored feed").expect("feed must exist");
    assert_eq!(stored_feed.title.as_deref(), Some("Example Feed"));
    assert_eq!(stored_feed.description.as_deref(), Some("Example description"));
    assert_eq!(stored_feed.site_url.as_ref().map(|url| url.as_str()), Some("https://example.com/"));

    let entries = library.entries.list_entries(&EntryQuery::default()).await.expect("list entries");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].title, "Hello World Updated");
}

#[tokio::test]
async fn localized_writeback_does_not_override_newer_refresh_content() {
    let library = Library::memory().await;
    let feed = library
        .feeds
        .upsert_subscription(&subscription("https://example.com/feed.xml", "Example Feed"))
        .await
        .expect("create feed");

    let first_html = "<p>old</p>";
    let entry_url = "https://example.com/entry-1";
    library.seed(feed.id, vec![revision("entry-1", entry_url, "Entry", first_html)]).await;
    // The hash the store recorded for the first source revision.
    let first_hash =
        library.text("SELECT content_hash FROM entry_contents").await.expect("first source hash");

    let newer_html = "<p>new</p>";
    library.seed(feed.id, vec![revision("entry-1", entry_url, "Entry", newer_html)]).await;

    let (_, generation) =
        library.entries.active_entry_generation(1).await.expect("generation").expect("active");
    let localized_old_html = "<p>old<img src=\"data:image/png;base64,xxx\"></p>";
    let updated = write_localized(
        &library,
        feed.id,
        generation,
        "entry-1",
        Some(&first_hash),
        first_html,
        localized_old_html,
    )
    .await;

    assert!(!updated, "stale localization should not overwrite newer content");

    let stored = library.entries.get_entry(1).await.expect("load entry").expect("entry exists");
    assert_eq!(stored.content_html.as_deref(), Some(newer_html));
}

#[tokio::test]
async fn localized_writeback_from_old_generation_is_rejected_after_readd() {
    let library = Library::memory().await;
    let feed = library
        .feeds
        .upsert_subscription(&subscription("https://example.com/generation.xml", "Generation Feed"))
        .await
        .expect("create feed");

    let original_html = "<p>old generation</p>";
    let entry_url = "https://example.com/generation-entry";
    library
        .seed(
            feed.id,
            vec![revision("entry-generation", entry_url, "Generation Entry", original_html)],
        )
        .await;
    let old_hash =
        library.text("SELECT content_hash FROM entry_contents").await.expect("old source hash");

    let (_, old_generation) = library
        .entries
        .active_entry_generation(1)
        .await
        .expect("read original generation")
        .expect("active original entry");
    assert_eq!(old_generation, 0);

    library.feeds.set_deleted(feed.id, true).await.expect("delete feed");
    library
        .feeds
        .upsert_subscription(&subscription(feed.url.as_str(), "Re-added Generation Feed"))
        .await
        .expect("reactivate feed");

    let new_html = "<p>new generation</p>";
    library
        .seed(feed.id, vec![revision("entry-generation", entry_url, "Generation Entry", new_html)])
        .await;

    let (_, new_generation) = library
        .entries
        .active_entry_generation(1)
        .await
        .expect("read new generation")
        .expect("active re-added entry");
    assert_eq!(new_generation, 1);

    let localized_old_html = "<p>old generation<img src=\"data:image/png;base64,xxx\"></p>";
    let updated = write_localized(
        &library,
        feed.id,
        old_generation,
        "entry-generation",
        Some(&old_hash),
        original_html,
        localized_old_html,
    )
    .await;

    assert!(!updated, "old generation localization must not write into the re-added feed");
    let stored = library.entries.get_entry(1).await.expect("load entry").expect("entry exists");
    assert_eq!(stored.content_html.as_deref(), Some(new_html));
}
