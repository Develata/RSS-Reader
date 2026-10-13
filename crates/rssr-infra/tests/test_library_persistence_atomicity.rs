#![cfg(not(target_arch = "wasm32"))]

//! Atomicity of removal, reactivation, configuration replacement and late refresh writes,
//! against the v0.2 library store (ports of `test_persistence_atomicity.rs`).
//!
//! In one library file the bodies of an entry are removed with it (foreign-key cascade, same
//! transaction), so the v0.1.x cross-database content GC queue has no counterpart. The tests
//! that only exercised that queue are not ported; the invariant behind them is: removal and
//! configuration replacement are all-or-nothing and never leave a body without its entry.

#[path = "support/library.rs"]
mod library;

use library::Library;
use rssr_application::{
    ConfigReplacementFeed, ConfigReplacementPlan, ConfigReplacementPort, FeedRefreshUpdate,
    ParsedEntryData, ParsedFeedUpdate, RefreshCommit, RefreshStorePort, SubscriptionActivationPort,
    SubscriptionRemovalPort,
};
use rssr_domain::DomainError;
use rssr_domain::{
    AppStateRepository, Feed, FeedRepository, NewFeedSubscription, SettingsRepository, UserSettings,
};
use time::OffsetDateTime;
use url::Url;

fn subscription(url: &str, title: &str) -> NewFeedSubscription {
    NewFeedSubscription {
        site_url: None,
        url: Url::parse(url).unwrap(),
        title: Some(title.into()),
        source_title: None,
        folder: None,
    }
}

async fn add_feed(library: &Library, url: &str) -> Feed {
    library.feeds.upsert_subscription(&subscription(url, "Original")).await.unwrap()
}

fn entry(id: &str) -> ParsedEntryData {
    ParsedEntryData {
        url: Some(Url::parse(&format!("https://example.com/{id}")).unwrap()),
        title: format!("Entry {id}"),
        summary: Some("summary".into()),
        content_html: Some("<p>body</p>".into()),
        content_text: Some("body".into()),
        published_at: Some(OffsetDateTime::UNIX_EPOCH),
        ..library::entry(id)
    }
}

async fn entry_count(library: &Library) -> i64 {
    library.scalar("SELECT COUNT(*) FROM entries").await
}

async fn body_count(library: &Library) -> i64 {
    library.scalar("SELECT COUNT(*) FROM entry_contents").await
}

async fn orphan_body_count(library: &Library) -> i64 {
    library
        .scalar(
            "SELECT COUNT(*) FROM entry_contents WHERE entry_id NOT IN (SELECT id FROM entries)",
        )
        .await
}

#[tokio::test]
async fn activation_port_captures_generation_and_rejects_active_duplicate() {
    let library = Library::memory().await;
    let subscription = subscription("https://example.com/activation.xml", "Activation");

    let first = library.mutations.activate_subscription(subscription.clone()).await.unwrap();
    assert_eq!(first.generation, 0);

    let duplicate = library
        .mutations
        .activate_subscription(subscription.clone())
        .await
        .expect_err("active duplicate must be rejected under the activation lock");
    assert!(duplicate.to_string().contains("已订阅"));

    library.mutations.remove_subscription(first.feed.id, false).await.unwrap();
    let reactivated = library.mutations.activate_subscription(subscription).await.unwrap();
    assert_eq!(reactivated.feed.id, first.feed.id);
    assert_eq!(reactivated.generation, 1);
}

#[tokio::test]
async fn active_subscription_lookup_normalizes_url_and_ignores_tombstones() {
    let library = Library::memory().await;
    let feed = add_feed(&library, "https://example.com/feed.xml").await;
    let equivalent = Url::parse("https://example.com:443/feed.xml#fragment").unwrap();

    assert!(library.feeds.has_active_subscription_url(&equivalent).await.unwrap());
    library.mutations.remove_subscription(feed.id, false).await.unwrap();
    assert!(!library.feeds.has_active_subscription_url(&equivalent).await.unwrap());
}

/// Adapted for intentional difference 1: the old test asserted that the content GC queue rolled
/// back with the failed purge; the bodies of the entries are what must survive here.
#[tokio::test]
async fn index_delete_failure_rolls_back_tombstone_and_entries() {
    let library = Library::memory().await;
    let feed = add_feed(&library, "https://example.com/original.xml").await;
    library.seed(feed.id, vec![entry("one")]).await;

    library
        .exec(
            "CREATE TRIGGER fail_entry_delete
             BEFORE DELETE ON entries
             WHEN OLD.feed_id = 1
             BEGIN
                 SELECT RAISE(ABORT, 'forced entry delete failure');
             END",
        )
        .await;

    library.mutations.remove_subscription(feed.id, true).await.expect_err("forced failure");

    assert!(library.feeds.get_feed(feed.id).await.unwrap().is_some());
    assert_eq!(entry_count(&library).await, 1);
    assert_eq!(body_count(&library).await, 1, "bodies must roll back with the failed purge");
}

/// Adapted for intentional difference 1. v0.1.x: a failing content delete left the subscription
/// hidden with its bodies queued for a later retry. Now the bodies go with the entries in the
/// same transaction, so a failing body delete fails the whole removal and nothing is half done;
/// once the fault is gone the removal succeeds and no body is left.
#[tokio::test]
async fn content_cleanup_failure_does_not_expose_half_deleted_subscription() {
    let library = Library::memory().await;
    let feed = add_feed(&library, "https://example.com/content-cleanup.xml").await;
    library.seed(feed.id, vec![entry("one")]).await;

    library
        .exec(
            "CREATE TRIGGER fail_content_delete
             BEFORE DELETE ON entry_contents
             BEGIN
                 SELECT RAISE(ABORT, 'forced content cleanup failure');
             END",
        )
        .await;

    library.mutations.remove_subscription(feed.id, true).await.expect_err("forced failure");

    assert!(library.feeds.get_feed(feed.id).await.unwrap().is_some(), "still subscribed");
    assert_eq!(entry_count(&library).await, 1);
    assert_eq!(body_count(&library).await, 1);
    assert_eq!(orphan_body_count(&library).await, 0);

    library.exec("DROP TRIGGER fail_content_delete").await;
    library.mutations.remove_subscription(feed.id, true).await.unwrap();
    assert!(library.feeds.get_feed(feed.id).await.unwrap().is_none());
    assert_eq!(entry_count(&library).await, 0);
    assert_eq!(body_count(&library).await, 0, "the retried removal purges every body");
}

/// Invariant behind `orphan_cleanup_removes_failed_purge_content_after_reactivation`: whatever
/// happened to an earlier purge, reactivating the subscription finds none of its old entries or
/// bodies (there is no orphan to collect), and the old work is fenced out by the generation.
#[tokio::test]
async fn reactivation_after_purge_starts_without_old_entries_or_bodies() {
    let library = Library::memory().await;
    let feed = add_feed(&library, "https://example.com/reactivated-orphan.xml").await;
    library.seed(feed.id, vec![entry("one")]).await;
    let stale_target = library.refresh.get_target(feed.id).await.unwrap().unwrap();

    library.mutations.remove_subscription(feed.id, true).await.unwrap();
    assert_eq!(orphan_body_count(&library).await, 0);

    let reactivated = library
        .mutations
        .activate_subscription(subscription(feed.url.as_str(), "Reactivated"))
        .await
        .unwrap();
    assert_eq!(reactivated.feed.id, feed.id);
    assert_eq!(reactivated.generation, 1);
    assert_eq!(entry_count(&library).await, 0);
    assert_eq!(body_count(&library).await, 0);

    let late = library
        .refresh
        .commit(&stale_target, library::updated(vec![entry("late")]))
        .await
        .expect_err("work started for the old subscription must not write into the new one");
    assert!(matches!(late.downcast_ref::<DomainError>(), Some(DomainError::NotFound)));
    assert_eq!(entry_count(&library).await, 0);
    assert_eq!(body_count(&library).await, 0);
}

#[tokio::test]
async fn orphan_cleanup_preserves_non_purge_deleted_feed_content() {
    let library = Library::memory().await;
    let feed = add_feed(&library, "https://example.com/keep-content.xml").await;
    library.seed(feed.id, vec![entry("one")]).await;

    library.mutations.remove_subscription(feed.id, false).await.unwrap();

    assert_eq!(entry_count(&library).await, 1);
    assert_eq!(body_count(&library).await, 1);
    assert_eq!(orphan_body_count(&library).await, 0);
}

#[tokio::test]
async fn config_replacement_failure_rolls_back_every_library_change() {
    let library = Library::memory().await;
    let original = add_feed(&library, "https://example.com/original.xml").await;
    let original_settings = UserSettings::default();
    library.settings.save(&original_settings).await.unwrap();

    library
        .exec(
            "CREATE TRIGGER fail_config_insert
             BEFORE INSERT ON feeds
             WHEN NEW.url = 'https://example.com/fail.xml'
             BEGIN
                 SELECT RAISE(ABORT, 'forced config failure');
             END",
        )
        .await;

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

    library.mutations.replace_config(plan).await.expect_err("forced failure");

    let persisted = library.feeds.list_feeds().await.unwrap();
    assert_eq!(persisted.len(), 1);
    assert_eq!(persisted[0].id, original.id);
    assert_eq!(persisted[0].url.as_str(), "https://example.com/original.xml");
    assert_eq!(library.settings.load().await.unwrap(), original_settings);
}

/// Invariant behind `config_replacement_durably_queues_content_cleanup` (intentional
/// difference 1). v0.1.x: a failing content delete during replacement left the removal done and
/// the cleanup queued. Now replacement is one transaction including the bodies: a failing body
/// delete leaves the previous configuration entirely in place, and after the fault is gone the
/// replacement removes the feed, its entries and its bodies together.
#[tokio::test]
async fn config_replacement_removes_bodies_with_entries_or_changes_nothing() {
    let library = Library::memory().await;
    let feed = add_feed(&library, "https://example.com/config-purge.xml").await;
    library.seed(feed.id, vec![entry("one")]).await;
    let original_settings = UserSettings::default();
    let changed_settings = UserSettings { refresh_interval_minutes: 99, ..UserSettings::default() };

    library
        .exec(
            "CREATE TRIGGER fail_content_delete
             BEFORE DELETE ON entry_contents
             BEGIN
                 SELECT RAISE(ABORT, 'forced config content cleanup failure');
             END",
        )
        .await;

    library
        .mutations
        .replace_config(ConfigReplacementPlan {
            feeds: Vec::new(),
            settings: changed_settings.clone(),
        })
        .await
        .expect_err("forced failure");
    assert!(library.feeds.get_feed(feed.id).await.unwrap().is_some());
    assert_eq!(entry_count(&library).await, 1);
    assert_eq!(body_count(&library).await, 1);
    assert_eq!(library.settings.load().await.unwrap(), original_settings);

    library.exec("DROP TRIGGER fail_content_delete").await;
    let outcome = library
        .mutations
        .replace_config(ConfigReplacementPlan {
            feeds: Vec::new(),
            settings: changed_settings.clone(),
        })
        .await
        .unwrap();
    assert_eq!(outcome.removed_feed_count, 1);
    assert!(library.feeds.get_feed(feed.id).await.unwrap().is_none());
    assert_eq!(entry_count(&library).await, 0);
    assert_eq!(body_count(&library).await, 0);
    assert_eq!(library.settings.load().await.unwrap(), changed_settings);
}

#[tokio::test]
async fn config_replacement_derives_removals_from_locked_current_state() {
    let library = Library::memory().await;
    let retained = add_feed(&library, "https://example.com/retained.xml").await;
    let late = add_feed(&library, "https://example.com/late.xml").await;

    let outcome = library
        .mutations
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
    let active = library.feeds.list_feeds().await.unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].id, retained.id);
    assert_eq!(active[0].title, None, "replacement metadata is exact desired state");
    assert!(library.feeds.get_feed(late.id).await.unwrap().is_none());
}

/// The old test called `update_feed_metadata` on the feed repository; metadata now only changes
/// through a refresh commit, which carries the target (and its generation) the work started for.
#[tokio::test]
async fn deleted_feed_rejects_late_metadata_update() {
    let library = Library::memory().await;
    let feed = add_feed(&library, "https://example.com/metadata-race.xml").await;
    let target = library.refresh.get_target(feed.id).await.unwrap().unwrap();
    library.mutations.remove_subscription(feed.id, false).await.unwrap();

    let error = library
        .refresh
        .commit(
            &target,
            RefreshCommit::Updated {
                update: FeedRefreshUpdate {
                    metadata: Default::default(),
                    feed: ParsedFeedUpdate {
                        title: Some("Late title".into()),
                        site_url: Some(Url::parse("https://late.example.com/").unwrap()),
                        description: Some("late description".into()),
                        entries: Vec::new(),
                    },
                },
            },
        )
        .await
        .expect_err("deleted feed must reject late metadata");
    assert!(matches!(error.downcast_ref::<DomainError>(), Some(DomainError::NotFound)));

    // 添加时的名称在 `custom_title`；源标题列没有被迟到的元数据写入（审计 B6）。
    let id = feed.id;
    assert_eq!(
        library.text(format!("SELECT custom_title FROM feeds WHERE id = {id}")).await.as_deref(),
        Some("Original")
    );
    assert_eq!(library.text(format!("SELECT title FROM feeds WHERE id = {id}")).await, None);
    assert_eq!(library.text(format!("SELECT site_url FROM feeds WHERE id = {id}")).await, None);
    assert_eq!(library.text(format!("SELECT description FROM feeds WHERE id = {id}")).await, None);
}

/// The old test called `upsert_entries_with_outcome` directly; entries now only arrive through a
/// refresh commit.
#[tokio::test]
async fn deleted_feed_rejects_late_index_upsert() {
    let library = Library::memory().await;
    let feed = add_feed(&library, "https://example.com/race.xml").await;
    let target = library.refresh.get_target(feed.id).await.unwrap().unwrap();
    library.mutations.remove_subscription(feed.id, true).await.unwrap();

    let error = library
        .refresh
        .commit(&target, library::updated(vec![entry("late")]))
        .await
        .expect_err("deleted feed must reject late refresh");

    assert!(matches!(error.downcast_ref::<DomainError>(), Some(DomainError::NotFound)));
    assert_eq!(entry_count(&library).await, 0);
    assert_eq!(body_count(&library).await, 0);
}

/// 读不懂的应用状态（例如新版本写入的字段）不能让删除订阅整体失败（审计 A5）。
#[tokio::test]
async fn unreadable_app_state_does_not_block_subscription_removal() {
    let library = Library::memory().await;
    let feed = library
        .feeds
        .upsert_subscription(&library::subscription("https://example.com/feed.xml"))
        .await
        .expect("subscribe");
    let newer_version_state =
        format!(r#"{{"last_opened_feed_id":{},"written_by_a_newer_version":true}}"#, feed.id);
    library
        .exec(format!(
            "INSERT INTO app_settings (key, value, updated_at)
             VALUES ('app_state_v2', '{newer_version_state}', '2026-10-09T00:00:00Z')"
        ))
        .await;
    assert!(library.app_state.load().await.is_err(), "the seeded state is unreadable");

    library
        .mutations
        .remove_subscription(feed.id, true)
        .await
        .expect("removing a subscription must not depend on parsing the app state");
    assert!(library.feeds.get_feed(feed.id).await.expect("get feed").is_none());
}

/// Only unreadable app-state JSON is skipped; a failing read of the app state fails the removal,
/// which then changes nothing.
#[tokio::test]
async fn an_app_state_read_failure_rolls_the_removal_back() {
    let library = Library::memory().await;
    let feed = add_feed(&library, "https://example.com/state-read.xml").await;
    library.seed(feed.id, vec![entry("kept")]).await;
    library.exec("ALTER TABLE app_settings RENAME TO app_settings_unavailable").await;

    let removed = library.mutations.remove_subscription(feed.id, true).await;
    assert!(removed.is_err(), "a storage failure is not an unreadable document");
    library.exec("ALTER TABLE app_settings_unavailable RENAME TO app_settings").await;
    assert!(library.feeds.get_feed(feed.id).await.unwrap().is_some());
    assert_eq!((entry_count(&library).await, body_count(&library).await), (1, 1));
}

/// A time that RFC 3339 cannot represent fails the refresh commit as a whole: nothing of it is
/// stored, the library stays readable and the last success does not advance.
#[tokio::test]
async fn an_unstorable_entry_time_fails_the_whole_commit() {
    use rssr_domain::{EntryIndexRepository, EntryQuery};
    use time::{Date, Month, PrimitiveDateTime, Time};

    let library = Library::memory().await;
    let feed = add_feed(&library, "https://example.com/time.xml").await;
    library.seed(feed.id, vec![entry("before")]).await;
    let target = library.refresh.get_target(feed.id).await.unwrap().unwrap();
    let last_success = library.feeds.get_feed(feed.id).await.unwrap().unwrap().last_success_at;

    let year_minus_one = PrimitiveDateTime::new(
        Date::from_calendar_date(-1, Month::January, 1).unwrap(),
        Time::MIDNIGHT,
    )
    .assume_utc();
    let commit = RefreshCommit::Updated {
        update: FeedRefreshUpdate {
            metadata: Default::default(),
            feed: ParsedFeedUpdate {
                title: Some("Changed".into()),
                site_url: None,
                description: None,
                entries: vec![
                    entry("fine"),
                    ParsedEntryData { published_at: Some(year_minus_one), ..entry("ancient") },
                ],
            },
        },
    };
    assert!(library.refresh.commit(&target, commit).await.is_err());

    let listed = library.entries.list_entries(&EntryQuery::default()).await.unwrap();
    assert_eq!(listed.len(), 1, "only the entry stored before remains");
    let stored = library.feeds.get_feed(feed.id).await.unwrap().unwrap();
    assert_eq!(stored.source_title, None, "feed metadata rolled back too");
    assert_eq!(stored.last_success_at, last_success);
    assert!(stored.fetch_error.is_some(), "the failure is recorded");
}
