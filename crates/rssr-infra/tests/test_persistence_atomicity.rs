use std::{
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use rssr_application::{
    ConfigReplacementFeed, ConfigReplacementPlan, ConfigReplacementPort,
    SubscriptionActivationPort, SubscriptionRemovalPort,
};
use rssr_domain::{FeedRepository, NewFeedSubscription, SettingsRepository, UserSettings};
use rssr_infra::{
    application_adapters::{SqlitePersistenceMutations, cleanup_pending_entry_content},
    db::{
        entry_repository::SqliteEntryRepository, feed_repository::SqliteFeedRepository, migrate,
        migrate_content, settings_repository::SqliteSettingsRepository,
        sqlite_native::NativeSqliteBackend, storage_backend::StorageBackend,
    },
    parser::{ParsedEntry, ParsedFeed},
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
async fn activation_port_captures_generation_and_rejects_active_duplicate() {
    let (_index_pool, _content_pool, _feeds, _entries, _settings, mutations) = fixture().await;
    let subscription = NewFeedSubscription {
        site_url: None,
        url: Url::parse("https://example.com/activation.xml").unwrap(),
        title: Some("Activation".into()),
        folder: None,
    };

    let first = mutations.activate_subscription(subscription.clone()).await.unwrap();
    assert_eq!(first.generation, 0);

    let duplicate = mutations
        .activate_subscription(subscription.clone())
        .await
        .expect_err("active duplicate must be rejected under the activation lock");
    assert!(duplicate.to_string().contains("已订阅"));

    mutations.remove_subscription(first.feed.id, false).await.unwrap();
    let reactivated = mutations.activate_subscription(subscription).await.unwrap();
    assert_eq!(reactivated.feed.id, first.feed.id);
    assert_eq!(reactivated.generation, 1);
}

#[tokio::test]
async fn active_subscription_lookup_normalizes_url_and_ignores_tombstones() {
    let (_index_pool, _content_pool, feeds, _entries, _settings, mutations) = fixture().await;
    let feed = add_feed(&feeds, "https://example.com/feed.xml").await;
    let equivalent = Url::parse("https://example.com:443/feed.xml#fragment").unwrap();

    assert!(feeds.has_active_subscription_url(&equivalent).await.unwrap());
    mutations.remove_subscription(feed.id, false).await.unwrap();
    assert!(!feeds.has_active_subscription_url(&equivalent).await.unwrap());
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
    let queued: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM content_gc_queue")
        .fetch_one(&index_pool)
        .await
        .unwrap();
    assert_eq!(queued, 0, "cleanup queue must roll back with the failed purge");
}

#[tokio::test]
async fn content_cleanup_failure_does_not_expose_half_deleted_subscription() {
    let (index_pool, content_pool, feeds, entries, _settings, mutations) = fixture().await;
    cleanup_pending_entry_content(&index_pool, &content_pool).await.unwrap();
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
    assert_eq!(remaining, 1, "failed cache cleanup remains hidden but pending");
    let queued: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM content_gc_queue")
        .fetch_one(&index_pool)
        .await
        .unwrap();
    assert_eq!(queued, 1, "failed content cleanup must remain durably queued");

    sqlx::query("DROP TRIGGER fail_content_delete").execute(&content_pool).await.unwrap();
    let removed = cleanup_pending_entry_content(&index_pool, &content_pool).await.unwrap();
    assert_eq!(removed, 1);
    let remaining: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM entry_contents WHERE feed_id = ?1")
            .bind(feed.id)
            .fetch_one(&content_pool)
            .await
            .unwrap();
    assert_eq!(remaining, 0, "startup retry must eventually purge deleted-feed content");
    let queued: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM content_gc_queue")
        .fetch_one(&index_pool)
        .await
        .unwrap();
    assert_eq!(queued, 0);
}

#[tokio::test]
async fn orphan_cleanup_removes_failed_purge_content_after_reactivation() {
    let (index_pool, content_pool, feeds, entries, _settings, mutations) = fixture().await;
    cleanup_pending_entry_content(&index_pool, &content_pool).await.unwrap();
    let feed = add_feed(&feeds, "https://example.com/reactivated-orphan.xml").await;
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
    sqlx::query("DROP TRIGGER fail_content_delete").execute(&content_pool).await.unwrap();

    let reactivated = mutations
        .activate_subscription(NewFeedSubscription {
            site_url: None,
            url: feed.url.clone(),
            title: Some("Reactivated".into()),
            folder: None,
        })
        .await
        .unwrap();
    assert_eq!(reactivated.feed.id, feed.id);
    assert_eq!(reactivated.generation, 1);
    let queued: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM content_gc_queue")
        .fetch_one(&index_pool)
        .await
        .unwrap();
    assert_eq!(queued, 1, "reactivation must not discard the old cleanup obligation");

    let removed = cleanup_pending_entry_content(&index_pool, &content_pool).await.unwrap();
    assert_eq!(removed, 1, "reactivation must not hide old orphaned content from GC");
    let remaining: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM entry_contents WHERE feed_id = ?1")
            .bind(feed.id)
            .fetch_one(&content_pool)
            .await
            .unwrap();
    assert_eq!(remaining, 0);
    assert!(feeds.get_feed(feed.id).await.unwrap().is_some());
}

#[tokio::test]
async fn orphan_cleanup_preserves_non_purge_deleted_feed_content() {
    let (index_pool, content_pool, feeds, entries, _settings, mutations) = fixture().await;
    let feed = add_feed(&feeds, "https://example.com/keep-content.xml").await;
    entries.upsert_entries(feed.id, &[entry("one")]).await.unwrap();

    mutations.remove_subscription(feed.id, false).await.unwrap();
    let removed = cleanup_pending_entry_content(&index_pool, &content_pool).await.unwrap();

    assert_eq!(removed, 0);
    assert!(entries.has_entries_for_feed(feed.id).await.unwrap());
    let remaining: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM entry_contents WHERE feed_id = ?1")
            .bind(feed.id)
            .fetch_one(&content_pool)
            .await
            .unwrap();
    assert_eq!(remaining, 1);
}

#[tokio::test]
async fn immediate_orphan_cleanup_does_not_block_reactivation() {
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let base = std::env::temp_dir().join(format!("rssr-immediate-purge-race-{nonce}"));
    std::fs::create_dir_all(&base).unwrap();
    let backend = NativeSqliteBackend::with_path(base.join("rss-reader.db"));
    let index_pool = backend.connect().await.unwrap();
    backend.migrate(&index_pool).await.unwrap();
    let content_pool = backend.connect_content().await.unwrap();
    backend.migrate_content(&content_pool).await.unwrap();
    cleanup_pending_entry_content(&index_pool, &content_pool).await.unwrap();

    let feeds = Arc::new(SqliteFeedRepository::new(index_pool.clone()));
    let entries =
        SqliteEntryRepository::new_with_content_pool(index_pool.clone(), content_pool.clone());
    let mutations = SqlitePersistenceMutations::new(index_pool.clone(), content_pool.clone());
    let feed = add_feed(&feeds, "https://example.com/immediate-race.xml").await;
    entries.upsert_entries(feed.id, &[entry("one")]).await.unwrap();

    let content_blocker = content_pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
    let removal = {
        let mutations = mutations.clone();
        tokio::spawn(async move { mutations.remove_subscription(feed.id, true).await })
    };
    tokio::time::sleep(Duration::from_millis(50)).await;

    let reactivation_subscription = NewFeedSubscription {
        site_url: None,
        url: Url::parse("https://example.com/immediate-race.xml").unwrap(),
        title: Some("Reactivated".into()),
        folder: None,
    };
    let reactivation = {
        let mutations = mutations.clone();
        tokio::spawn(
            async move { mutations.activate_subscription(reactivation_subscription).await },
        )
    };
    let reactivated = tokio::time::timeout(Duration::from_secs(2), reactivation)
        .await
        .expect("reactivation must not wait for orphan-content deletion")
        .unwrap()
        .unwrap();
    assert_eq!(reactivated.generation, 1);
    assert!(!removal.is_finished(), "removal is still waiting on the blocked content database");

    content_blocker.rollback().await.unwrap();
    removal.await.unwrap().unwrap();
    let remaining: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM entry_contents WHERE feed_id = ?1")
            .bind(feed.id)
            .fetch_one(&content_pool)
            .await
            .unwrap();
    assert_eq!(remaining, 0);

    index_pool.close().await;
    content_pool.close().await;
    std::fs::remove_dir_all(base).unwrap();
}

#[tokio::test]
async fn orphan_cleanup_does_not_hold_index_writer_lock() {
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let base = std::env::temp_dir().join(format!("rssr-content-gc-race-{nonce}"));
    std::fs::create_dir_all(&base).unwrap();
    let backend = NativeSqliteBackend::with_path(base.join("rss-reader.db"));
    let index_pool = backend.connect().await.unwrap();
    backend.migrate(&index_pool).await.unwrap();
    let content_pool = backend.connect_content().await.unwrap();
    backend.migrate_content(&content_pool).await.unwrap();

    let feeds = Arc::new(SqliteFeedRepository::new(index_pool.clone()));
    let entries =
        SqliteEntryRepository::new_with_content_pool(index_pool.clone(), content_pool.clone());
    let feed = add_feed(&feeds, "https://example.com/gc-race.xml").await;
    entries.upsert_entries(feed.id, &[entry("one")]).await.unwrap();

    sqlx::query("UPDATE feeds SET is_deleted = 1 WHERE id = ?1")
        .bind(feed.id)
        .execute(&index_pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM entries WHERE feed_id = ?1")
        .bind(feed.id)
        .execute(&index_pool)
        .await
        .unwrap();

    let content_blocker = content_pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
    let cleanup_index = index_pool.clone();
    let cleanup_content = content_pool.clone();
    let cleanup = tokio::spawn(async move {
        cleanup_pending_entry_content(&cleanup_index, &cleanup_content).await
    });
    tokio::time::sleep(Duration::from_millis(50)).await;

    let reactivation_subscription = NewFeedSubscription {
        site_url: None,
        url: feed.url.clone(),
        title: Some("Reactivated".into()),
        folder: None,
    };
    let reactivation_feeds = feeds.clone();
    let reactivation = tokio::spawn(async move {
        reactivation_feeds.upsert_subscription(&reactivation_subscription).await
    });
    let reactivated = tokio::time::timeout(Duration::from_secs(2), reactivation)
        .await
        .expect("feed reactivation must not wait for content GC")
        .unwrap()
        .unwrap();
    assert_eq!(reactivated.id, feed.id);
    assert!(!cleanup.is_finished(), "cleanup should still be waiting on the content writer");

    content_blocker.rollback().await.unwrap();
    cleanup.await.unwrap().unwrap();

    let remaining: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM entry_contents WHERE feed_id = ?1")
            .bind(feed.id)
            .fetch_one(&content_pool)
            .await
            .unwrap();
    assert_eq!(remaining, 0);

    index_pool.close().await;
    content_pool.close().await;
    std::fs::remove_dir_all(base).unwrap();
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

    let changed_settings = UserSettings { refresh_interval_minutes: 99, ..UserSettings::default() };
    let plan = ConfigReplacementPlan {
        feeds: vec![
            ConfigReplacementFeed {
                url: Url::parse("https://example.com/new.xml").unwrap(),
                title: Some("New".into()),
                folder: None,
            },
            ConfigReplacementFeed {
                url: Url::parse("https://example.com/fail.xml").unwrap(),
                title: Some("Fail".into()),
                folder: None,
            },
        ],
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
async fn config_replacement_durably_queues_content_cleanup() {
    let (index_pool, content_pool, feeds, entries, _settings, mutations) = fixture().await;
    cleanup_pending_entry_content(&index_pool, &content_pool).await.unwrap();
    let feed = add_feed(&feeds, "https://example.com/config-purge.xml").await;
    entries.upsert_entries(feed.id, &[entry("one")]).await.unwrap();

    sqlx::query(
        r#"
        CREATE TRIGGER fail_content_delete
        BEFORE DELETE ON entry_contents
        BEGIN
            SELECT RAISE(ABORT, 'forced config content cleanup failure');
        END
        "#,
    )
    .execute(&content_pool)
    .await
    .unwrap();

    let outcome = mutations
        .replace_config(ConfigReplacementPlan {
            feeds: Vec::new(),
            settings: UserSettings::default(),
        })
        .await
        .unwrap();
    assert_eq!(outcome.removed_feed_count, 1);
    assert!(feeds.get_feed(feed.id).await.unwrap().is_none());
    assert!(!entries.has_entries_for_feed(feed.id).await.unwrap());

    let queued: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM content_gc_queue")
        .fetch_one(&index_pool)
        .await
        .unwrap();
    assert_eq!(queued, 1);

    sqlx::query("DROP TRIGGER fail_content_delete").execute(&content_pool).await.unwrap();
    let removed = cleanup_pending_entry_content(&index_pool, &content_pool).await.unwrap();
    assert_eq!(removed, 1);
    let queued: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM content_gc_queue")
        .fetch_one(&index_pool)
        .await
        .unwrap();
    assert_eq!(queued, 0);
}

#[tokio::test]
async fn config_replacement_derives_removals_from_locked_current_state() {
    let (_index_pool, _content_pool, feeds, _entries, _settings, mutations) = fixture().await;
    let retained = add_feed(&feeds, "https://example.com/retained.xml").await;
    let late = add_feed(&feeds, "https://example.com/late.xml").await;

    let outcome = mutations
        .replace_config(ConfigReplacementPlan {
            feeds: vec![ConfigReplacementFeed {
                url: retained.url.clone(),
                title: None,
                folder: None,
            }],
            settings: UserSettings::default(),
        })
        .await
        .unwrap();

    assert_eq!(outcome.removed_feed_count, 1);
    assert!(!outcome.settings_updated);
    let active = feeds.list_feeds().await.unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].id, retained.id);
    assert_eq!(active[0].title, None, "replacement metadata is exact desired state");
    assert!(feeds.get_feed(late.id).await.unwrap().is_none());
}

#[tokio::test]
async fn deleted_feed_rejects_late_metadata_update() {
    let (index_pool, _content_pool, feeds, _entries, _settings, mutations) = fixture().await;
    let feed = add_feed(&feeds, "https://example.com/metadata-race.xml").await;
    mutations.remove_subscription(feed.id, false).await.unwrap();

    let error = feeds
        .update_feed_metadata(
            feed.id,
            &ParsedFeed {
                title: Some("Late title".into()),
                site_url: Some(Url::parse("https://late.example.com/").unwrap()),
                description: Some("late description".into()),
                entries: Vec::new(),
            },
        )
        .await
        .expect_err("deleted feed must reject late metadata");
    assert!(matches!(error, rssr_domain::DomainError::NotFound));

    let title: Option<String> = sqlx::query_scalar("SELECT title FROM feeds WHERE id = ?1")
        .bind(feed.id)
        .fetch_one(&index_pool)
        .await
        .unwrap();
    assert_eq!(title.as_deref(), Some("Original"));
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
