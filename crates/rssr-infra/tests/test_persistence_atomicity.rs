use std::sync::Arc;

use rssr_application::{
    ConfigReplacementPlan, ConfigReplacementPort, SubscriptionRemovalPort,
};
use rssr_domain::{FeedRepository, NewFeedSubscription, SettingsRepository, UserSettings};
use rssr_infra::{
    application_adapters::SqlitePersistenceMutations,
    db::{
        entry_repository::SqliteEntryRepository, feed_repository::SqliteFeedRepository, migrate,
        migrate_content, settings_repository::SqliteSettingsRepository,
    },
    parser::ParsedEntry,
};
use time::OffsetDateTime;
use url::Url;

async fn fixture() -> (
    rssr_infra::db::SqlitePool,
    rssr_infra::db::SqlitePool,
    Arc<SqliteFeedRepository>,
    Arc<SqliteEntryRepository>,
    Arc<SqliteSettingsRepository>,
    SqlitePersistenceMutations,
) {
    let index_pool = rssr_infra::db::create_sqlite_pool("sqlite::memory:").await.unwrap();
    migrate(&index_pool).await.unwrap();
    let content_pool = rssr_infra::db::create_sqlite_pool("sqlite::memory:").await.unwrap();
    migrate_content(&content_pool).await.unwrap();

    let feeds = Arc::new(SqliteFeedRepository::new(index_pool.clone()));
    let entries = Arc::new(SqliteEntryRepository::new_with_content_pool(
        index_pool.clone(),
        content_pool.clone(),
    ));
    let settings = Arc::new(SqliteSettingsRepository::new(index_pool.clone()));
    let mutations = SqlitePersistenceMutations::new(index_pool.clone(), content_pool.clone());
    (index_pool, content_pool, feeds, entries, settings, mutations)
}

async fn add_feed(feeds: &SqliteFeedRepository, url: &str) -> rssr_domain::Feed {
    feeds
        .upsert_subscription(&NewFeedSubscription {
            site_url: None,
            url: Url::parse(url).unwrap(),
            title: Some("Original".into()),
            folder: None,
        })
        .await
        .unwrap()
}

fn entry(id: &str) -> ParsedEntry {
    ParsedEntry {
        external_id: id.into(),
        dedup_key: id.into(),
        url: Some(Url::parse(&format!("https://example.com/{id}")).unwrap()),
        title: format!("Entry {id}"),
        author: None,
        summary: Some("summary".into()),
        content_html: Some("<p>body</p>".into()),
        content_text: Some("body".into()),
        published_at: Some(OffsetDateTime::UNIX_EPOCH),
        updated_at_source: None,
    }
}

#[tokio::test]
async fn index_delete_failure_rolls_back_tombstone_and_entries() {
    let (index_pool, _content_pool, feeds, entries, _settings, mutations) = fixture().await;
    let feed = add_feed(&feeds, "https://example.com/original.xml").await;
    entries.upsert_entries(feed.id, &[entry("one")]).await.unwrap();

    sqlx::query(
        r#"
        CREATE TRIGGER fail_entry_delete
        BEFORE DELETE ON entries
        WHEN OLD.feed_id = 1
        BEGIN
            SELECT RAISE(ABORT, 'forced entry delete failure');
        END
        "#,
    )
    .execute(&index_pool)
    .await
    .unwrap();

    mutations.remove_subscription(feed.id, true).await.expect_err("forced failure");

    assert!(feeds.get_feed(feed.id).await.unwrap().is_some());
    assert!(entries.has_entries_for_feed(feed.id).await.unwrap());
}

#[tokio::test]
async fn content_cleanup_failure_does_not_expose_half_deleted_subscription() {
    let (_index_pool, content_pool, feeds, entries, _settings, mutations) = fixture().await;
    let feed = add_feed(&feeds, "https://example.com/content-cleanup.xml").await;
    entries.upsert_entries(feed.id, &[entry("one")]).await.unwrap();

    sqlx::query(
        r#"
        CREATE TRIGGER fail_content_delete
        BEFORE DELETE ON entry_contents
        WHEN OLD.feed_id = 1
        BEGIN
            SELECT RAISE(ABORT, 'forced content cleanup failure');
        END
        "#,
    )
    .execute(&content_pool)
    .await
    .unwrap();

    mutations.remove_subscription(feed.id, true).await.unwrap();

    assert!(feeds.get_feed(feed.id).await.unwrap().is_none());
    assert!(!entries.has_entries_for_feed(feed.id).await.unwrap());
    let remaining: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM entry_contents WHERE feed_id = ?1")
            .bind(feed.id)
            .fetch_one(&content_pool)
            .await
            .unwrap();
    assert_eq!(remaining, 1, "failed cache cleanup is allowed to remain retryable");
}

#[tokio::test]
async fn config_replacement_failure_rolls_back_every_index_database_change() {
    let (index_pool, _content_pool, feeds, _entries, settings, mutations) = fixture().await;
    let original = add_feed(&feeds, "https://example.com/original.xml").await;
    let original_settings = UserSettings::default();
    settings.save(&original_settings).await.unwrap();

    sqlx::query(
        r#"
        CREATE TRIGGER fail_config_insert
        BEFORE INSERT ON feeds
        WHEN NEW.url = 'https://example.com/fail.xml'
        BEGIN
            SELECT RAISE(ABORT, 'forced config failure');
        END
        "#,
    )
    .execute(&index_pool)
    .await
    .unwrap();

    let changed_settings =
        UserSettings { refresh_interval_minutes: 99, ..UserSettings::default() };
    let plan = ConfigReplacementPlan {
        upserts: vec![
            NewFeedSubscription {
                site_url: None,
                url: Url::parse("https://example.com/new.xml").unwrap(),
                title: Some("New".into()),
                folder: None,
            },
            NewFeedSubscription {
                site_url: None,
                url: Url::parse("https://example.com/fail.xml").unwrap(),
                title: Some("Fail".into()),
                folder: None,
            },
        ],
        removed_feed_ids: vec![original.id],
        settings: changed_settings,
    };

    mutations.replace_config(plan).await.expect_err("forced failure");

    let persisted = feeds.list_feeds().await.unwrap();
    assert_eq!(persisted.len(), 1);
    assert_eq!(persisted[0].id, original.id);
    assert_eq!(persisted[0].url.as_str(), "https://example.com/original.xml");
    assert_eq!(settings.load().await.unwrap(), original_settings);
}

#[tokio::test]
async fn deleted_feed_rejects_late_index_upsert() {
    let (_index_pool, _content_pool, feeds, entries, _settings, mutations) = fixture().await;
    let feed = add_feed(&feeds, "https://example.com/race.xml").await;
    mutations.remove_subscription(feed.id, true).await.unwrap();

    let error = entries
        .upsert_entries_with_outcome(feed.id, &[entry("late")])
        .await
        .expect_err("deleted feed must reject late refresh");

    assert!(matches!(error, rssr_domain::DomainError::NotFound));
    assert!(!entries.has_entries_for_feed(feed.id).await.unwrap());
}
