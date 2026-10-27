#![cfg(not(target_arch = "wasm32"))]

//! A legacy URL-keyed row is promoted in place when a hex GUID becomes visible (v0.2 library
//! version of `test_guid_identity_transition`; the rows now arrive through refresh commits).

#[path = "support/library.rs"]
mod library;

use library::Library;
use rssr_application::ParsedEntryData;
use rssr_domain::{EntryContentRepository, EntryIndexRepository};
use time::OffsetDateTime;
use url::Url;

const GUID: &str = "0123456789abcdef0123456789abcdef";
const ARTICLE_URL: &str = "https://example.com/article";

fn parsed(external_id: &str, dedup_key: &str, body: &str) -> ParsedEntryData {
    ParsedEntryData {
        external_id: external_id.into(),
        dedup_key: dedup_key.into(),
        url: Some(Url::parse(ARTICLE_URL).unwrap()),
        title: "Stable article".into(),
        author: None,
        summary: Some(body.into()),
        content_html: Some(format!("<p>{body}</p>")),
        content_text: Some(body.into()),
        published_at: Some(OffsetDateTime::UNIX_EPOCH),
        updated_at_source: None,
    }
}

#[tokio::test]
async fn legacy_url_identity_is_promoted_in_place_when_hex_guid_becomes_visible() {
    let library = Library::memory().await;
    let feed = library.add_feed("https://example.com/feed.xml").await;

    library.seed(feed.id, vec![parsed(ARTICLE_URL, ARTICLE_URL, "legacy body")]).await;
    let original_id =
        library.scalar(format!("SELECT id FROM entries WHERE feed_id = {}", feed.id)).await;
    library.entries.set_read(original_id, true).await.unwrap();
    library.entries.set_starred(original_id, true).await.unwrap();

    library.seed(feed.id, vec![parsed(GUID, GUID, "new body")]).await;

    let feed_id = feed.id;
    let rows: Vec<(i64, String, String, i64, i64)> = library
        .store
        .read(move |conn| {
            let mut statement = conn.prepare(
                "SELECT id, external_id, dedup_key, is_read, is_starred
                 FROM entries WHERE feed_id = ?1",
            )?;
            let rows = statement
                .query_map([feed_id], |row| {
                    Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].0, original_id);
    assert_eq!(rows[0].1, GUID);
    assert_eq!(rows[0].2, GUID);
    assert_eq!(rows[0].3, 1);
    assert_eq!(rows[0].4, 1);

    let content = library.entries.get_content(original_id).await.unwrap().unwrap();
    assert_eq!(content.content_text.as_deref(), Some("new body"));
}
