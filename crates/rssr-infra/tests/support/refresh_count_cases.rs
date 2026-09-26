use rssr_application::{
    FeedRefreshUpdate, ParsedEntryData, ParsedFeedUpdate, RefreshCommit, RefreshFailure,
    RefreshStorePort,
};

fn update(items: &[(i64, &str)]) -> RefreshCommit {
    RefreshCommit::Updated {
        update: FeedRefreshUpdate {
            metadata: Default::default(),
            feed: ParsedFeedUpdate {
                title: None,
                site_url: None,
                description: None,
                entries: items
                    .iter()
                    .map(|(id, title)| ParsedEntryData {
                        external_id: id.to_string(),
                        dedup_key: id.to_string(),
                        url: None,
                        title: title.to_string(),
                        author: None,
                        summary: None,
                        content_html: Some(format!("<p>{title}</p>")),
                        content_text: Some(title.to_string()),
                        published_at: None,
                        updated_at_source: None,
                    })
                    .collect(),
            },
        },
    }
}

pub async fn verify_counts(store: &dyn RefreshStorePort, feed_id: i64) {
    store.begin_batch().await.unwrap();
    assert_eq!(
        store
            .commit(feed_id, update(&[(1, "one"), (1, "one again")]))
            .await
            .unwrap()
            .inserted_count,
        1
    );
    assert_eq!(store.commit(feed_id, update(&[(1, "one again")])).await.unwrap().inserted_count, 0);
    assert_eq!(
        store
            .commit(feed_id, update(&[(1, "changed body"), (2, "two"), (3, "three")]))
            .await
            .unwrap()
            .inserted_count,
        2
    );
    store.end_batch().await.unwrap();
    assert_eq!(
        store.commit(feed_id, update(&[(1, "changed body again")])).await.unwrap().inserted_count,
        0
    );
    assert_eq!(
        store
            .commit(feed_id, RefreshCommit::NotModified { metadata: Default::default() })
            .await
            .unwrap()
            .inserted_count,
        0
    );
    assert_eq!(
        store
            .commit(
                feed_id,
                RefreshCommit::Failed {
                    failure: RefreshFailure { message: "offline".into(), metadata: None }
                }
            )
            .await
            .unwrap()
            .inserted_count,
        0
    );
}
