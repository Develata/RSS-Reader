use rssr_application::{
    FeedRefreshUpdate, ParsedEntryData, ParsedFeedUpdate, RefreshCommit, RefreshStorePort,
};
use rssr_domain::{EntryContentRepository, EntryIndexRepository, EntryQuery};

fn update(title: &str, html: Option<&str>, text: Option<&str>) -> RefreshCommit {
    RefreshCommit::Updated {
        update: FeedRefreshUpdate {
            metadata: Default::default(),
            feed: ParsedFeedUpdate {
                title: None,
                site_url: None,
                description: None,
                entries: vec![ParsedEntryData {
                    external_id: "entry".into(),
                    dedup_key: "entry".into(),
                    url: None,
                    title: title.into(),
                    author: None,
                    summary: None,
                    content_html: html.map(str::to_owned),
                    content_text: text.map(str::to_owned),
                    published_at: None,
                    updated_at_source: None,
                }],
            },
        },
    }
}

// One contract exercises both SQLite and browser storage; repeated responses must retain the
// full record, whereas partial updates preserve absent fields and empty strings replace them.

/// Commits as a new refresh would: each response answers its own request (target), so stores that
/// order refreshes by ticket see a fresh one every time.
async fn commit_fresh(
    store: &dyn RefreshStorePort,
    feed_id: i64,
    commit: RefreshCommit,
) -> anyhow::Result<rssr_application::RefreshCommitOutcome> {
    let target = store.get_target(feed_id).await?.expect("active subscription");
    store.commit(&target, commit).await
}

pub async fn verify_content_changes(
    store: &dyn RefreshStorePort,
    index: &dyn EntryIndexRepository,
    contents: &dyn EntryContentRepository,
    feed_id: i64,
) {
    assert_eq!(
        commit_fresh(store, feed_id, update("Title", Some("c"), Some("ab")))
            .await
            .unwrap()
            .inserted_count,
        1
    );
    let id = index.list_entries(&EntryQuery::default()).await.unwrap()[0].id;
    let first = contents.get_content(id).await.unwrap().unwrap();
    commit_fresh(store, feed_id, update("Title", Some("c"), Some("ab"))).await.unwrap();
    assert_eq!(contents.get_content(id).await.unwrap().unwrap(), first);

    // Moving a boundary between text and HTML is a different source: the length-prefixed hash
    // tells the two apart (the old concatenating hash did not), so the body is rewritten.
    commit_fresh(store, feed_id, update("Title", Some("bc"), Some("a"))).await.unwrap();
    let repartitioned = contents.get_content(id).await.unwrap().unwrap();
    assert_ne!(repartitioned.content_hash, first.content_hash);
    assert_eq!(repartitioned.content_html.as_deref(), Some("bc"));
    assert_eq!(repartitioned.content_text.as_deref(), Some("a"));

    commit_fresh(store, feed_id, update("New title", Some("bc"), Some("a"))).await.unwrap();
    let retitled = contents.get_content(id).await.unwrap().unwrap();
    assert_ne!(retitled.content_hash, repartitioned.content_hash);

    for (html, text, expected_html, expected_text) in [
        (None, Some("replacement"), "bc", "replacement"),
        (Some(""), None, "", "replacement"),
        (Some(""), Some(""), "", ""),
        (None, None, "", ""),
    ] {
        assert_eq!(
            commit_fresh(store, feed_id, update("New title", html, text))
                .await
                .unwrap()
                .inserted_count,
            0
        );
        let changed = contents.get_content(id).await.unwrap().unwrap();
        assert_eq!(changed.content_html.as_deref(), Some(expected_html));
        assert_eq!(changed.content_text.as_deref(), Some(expected_text));
        commit_fresh(store, feed_id, update("New title", html, text)).await.unwrap();
        assert_eq!(contents.get_content(id).await.unwrap().unwrap(), changed);
    }
}
