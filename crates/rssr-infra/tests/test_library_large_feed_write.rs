#![cfg(not(target_arch = "wasm32"))]

//! Large and duplicate-bearing batches against the v0.2 library store (port of
//! `test_large_feed_write`). A batch now arrives as one refresh commit; the index and the bodies
//! live in the same file, so a body is counted through its entry's `feed_id`.

#[path = "support/library.rs"]
mod library;

use library::Library;
use rssr_application::ParsedEntryData;
use rssr_domain::EntryContentRepository;
use time::OffsetDateTime;
use url::Url;

const ENTRY_COUNT: usize = 33_000;

#[tokio::test]
async fn thirty_three_thousand_entry_feed_writes_index_and_content_completely() {
    let library = Library::memory().await;
    let feed = library.add_feed("https://example.com/huge.xml").await;

    let parsed = (0..ENTRY_COUNT)
        .map(|index| ParsedEntryData {
            external_id: format!("entry-{index}"),
            dedup_key: format!("entry-{index}"),
            url: Some(Url::parse(&format!("https://example.com/articles/{index}")).unwrap()),
            title: format!("Entry {index}"),
            author: None,
            summary: Some(format!("Summary {index}")),
            content_html: Some(format!("<p>Body {index}</p>")),
            content_text: Some(format!("Body {index}")),
            published_at: Some(OffsetDateTime::UNIX_EPOCH),
            updated_at_source: None,
        })
        .collect::<Vec<_>>();

    assert_eq!(library.seed(feed.id, parsed).await, ENTRY_COUNT as u64);

    let index_count =
        library.scalar(format!("SELECT COUNT(*) FROM entries WHERE feed_id = {}", feed.id)).await;
    let content_count = library
        .scalar(format!(
            "SELECT COUNT(*) FROM entry_contents
             WHERE entry_id IN (SELECT id FROM entries WHERE feed_id = {})",
            feed.id
        ))
        .await;
    let marked_count = library
        .scalar(format!(
            "SELECT COUNT(*) FROM entries WHERE feed_id = {} AND has_content = 1",
            feed.id
        ))
        .await;

    assert_eq!(index_count, ENTRY_COUNT as i64);
    assert_eq!(content_count, ENTRY_COUNT as i64);
    assert_eq!(marked_count, ENTRY_COUNT as i64);

    let entry_ids = (1..=ENTRY_COUNT as i64).collect::<Vec<_>>();
    EntryContentRepository::delete_for_entry_ids(&library.entries, &entry_ids)
        .await
        .expect("chunked content delete");
    let remaining = library.scalar("SELECT COUNT(*) FROM entry_contents").await;
    assert_eq!(remaining, 0);
}

#[tokio::test]
async fn batched_content_upsert_preserves_sequential_duplicate_semantics() {
    let library = Library::memory().await;
    let feed = library.add_feed("https://example.com/duplicates.xml").await;

    let mut first = ParsedEntryData {
        external_id: "same-id".into(),
        dedup_key: "same-id".into(),
        url: Some(Url::parse("https://example.com/article").unwrap()),
        title: "First".into(),
        author: None,
        summary: None,
        content_html: Some("<p>first</p>".into()),
        content_text: Some("first".into()),
        published_at: Some(OffsetDateTime::UNIX_EPOCH),
        updated_at_source: None,
    };
    let mut second = first.clone();
    second.title = "Second".into();
    second.content_html = Some("<p>second</p>".into());
    second.content_text = Some("second".into());

    library.seed(feed.id, vec![first.clone(), second]).await;

    let sql_html = "SELECT content_html FROM entry_contents";
    let sql_text = "SELECT content_text FROM entry_contents";
    assert_eq!(library.scalar("SELECT COUNT(*) FROM entry_contents").await, 1);
    assert_eq!(library.text(sql_html).await.as_deref(), Some("<p>second</p>"));
    assert_eq!(library.text(sql_text).await.as_deref(), Some("second"));

    first.content_html = Some("<p>third</p>".into());
    first.content_text = Some("third".into());
    library.seed(feed.id, vec![first]).await;

    assert_eq!(library.scalar("SELECT COUNT(*) FROM entry_contents").await, 1);
    assert_eq!(library.text(sql_html).await.as_deref(), Some("<p>third</p>"));
    assert_eq!(library.text(sql_text).await.as_deref(), Some("third"));
}
