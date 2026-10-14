#![cfg(not(target_arch = "wasm32"))]

//! One-way import of v0.1.x libraries (both released layouts) into a v0.2 library. Fixtures are
//! built with the released migration files, recorded the way sqlx records them.

#[path = "support/library.rs"]
mod library;

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use library::Library;
use rssr_domain::{
    AppStateRepository, EntriesWorkspaceState, EntryContentRepository, EntryGroupingPreference,
    EntryIndexRepository, EntryQuery, FeedRepository, ReadFilter, SettingsRepository,
    StarredFilter, UserSettings,
};
use rssr_infra::store::LibraryStore;
use rssr_infra::store::import::{Layout, V01Library, import_v01};
use rusqlite::{Connection, params};
use serde_json::{Value, json};
use sha2::{Digest, Sha384};

const INDEX: [&str; 9] = [
    include_str!("../../../migrations/0001_initial.sql"),
    include_str!("../../../migrations/0002_split_entry_content.sql"),
    include_str!("../../../migrations/0003_entry_sort_key_indexes.sql"),
    include_str!("../../../migrations/0004_drop_unused_entry_sort_index.sql"),
    include_str!("../../../migrations/0005_restore_entry_sort_index.sql"),
    include_str!("../../../migrations/0006_feed_generation.sql"),
    include_str!("../../../migrations/0007_content_gc_queue.sql"),
    include_str!("../../../migrations/0008_entry_title_search.sql"),
    include_str!("../../../migrations/0009_feed_custom_title.sql"),
];
const CONTENT: &str = include_str!("../../../migrations_content/0001_initial.sql");

struct TempDir(PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("rssr-import-{label} 中文-{nonce}"));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn target(&self) -> PathBuf {
        self.0.join("library.sqlite3")
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[derive(Clone, Copy)]
enum Endings {
    Lf,
    Crlf,
}

fn checksum(sql: &str, endings: Endings) -> Vec<u8> {
    let lf = sql.replace("\r\n", "\n");
    let text = match endings {
        Endings::Lf => lf,
        Endings::Crlf => lf.replace('\n', "\r\n"),
    };
    Sha384::digest(text.as_bytes()).to_vec()
}

/// Applies `migrations` and records them the way sqlx does.
fn migrate_like_sqlx(conn: &Connection, migrations: &[&str], endings: Endings) {
    conn.execute_batch(
        "CREATE TABLE _sqlx_migrations (
             version BIGINT PRIMARY KEY, description TEXT NOT NULL,
             installed_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
             success BOOLEAN NOT NULL, checksum BLOB NOT NULL, execution_time BIGINT NOT NULL)",
    )
    .unwrap();
    for (index, sql) in migrations.iter().enumerate() {
        conn.execute_batch(sql).unwrap();
        conn.execute(
            "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time)
             VALUES (?1, 'migration', TRUE, ?2, 0)",
            params![index as i64 + 1, checksum(sql, endings)],
        )
        .unwrap();
    }
}

const T: &str = "2026-03-01T10:00:00Z";

/// A v0.1.x library at index migration `version`: two feeds (one removed, its entries kept), four
/// entries (the last inserted one deleted again, so the AUTOINCREMENT mark is above the highest
/// id), bodies for entries 1 and 2, one orphan body, settings and app state. Inline entries without
/// a body still carry a title hash, as v0.1.0–v0.1.6 wrote them.
fn fixture(
    dir: &Path,
    version: usize,
    endings: Endings,
    settings: &Value,
    app_state: Option<&Value>,
) -> V01Library {
    let index_path = dir.join("rss-reader.db");
    let index = Connection::open(&index_path).unwrap();
    index.pragma_update(None, "journal_mode", "WAL").unwrap();
    migrate_like_sqlx(&index, &INDEX[..version], endings);
    index
        .execute_batch(&format!(
            "INSERT INTO feeds (id, url, title, folder, etag, is_deleted, created_at, updated_at)
             VALUES (1, 'https://a.example/feed.xml', 'Feed A', 'News', 'etag-a', 0, '{T}', '{T}'),
                    (2, 'https://b.example/feed.xml', 'Feed B', NULL, NULL, 1, '{T}', '{T}');"
        ))
        .unwrap();
    if version >= 6 {
        index.execute_batch("UPDATE feeds SET generation = 3 WHERE id = 1").unwrap();
    }
    let inline = version < 2;
    for (id, feed, title, read) in [
        (1, 1, "ÉCOLE Rust", 1),
        (2, 1, "Second", 0),
        (3, 2, "Removed feed entry", 0),
        (4, 1, "Gone", 0),
    ] {
        if inline {
            let body = (id <= 2).then(|| format!("<p>body {id}</p>"));
            index
                .execute(
                    "INSERT INTO entries (id, feed_id, external_id, dedup_key, title, content_html,
                         content_hash, first_seen_at, is_read, created_at, updated_at, published_at)
                     VALUES (?1, ?2, ?1, ?1, ?3, ?4, ?5, ?6, ?7, ?6, ?6, ?6)",
                    params![
                        id,
                        feed,
                        title,
                        body,
                        if body.is_some() { "hash" } else { "title-hash" },
                        T,
                        read
                    ],
                )
                .unwrap();
        } else {
            index
                .execute(
                    "INSERT INTO entries (id, feed_id, external_id, dedup_key, title, has_content,
                         first_seen_at, is_read, created_at, updated_at, published_at)
                     VALUES (?1, ?2, ?1, ?1, ?3, ?4, ?5, ?6, ?5, ?5, ?5)",
                    params![id, feed, title, i64::from(id <= 2), T, read],
                )
                .unwrap();
        }
    }
    index.execute("DELETE FROM entries WHERE id = 4", []).unwrap();
    index
        .execute(
            "INSERT INTO app_settings (key, value, updated_at) VALUES ('user_settings', ?1, ?2)",
            params![settings.to_string(), T],
        )
        .unwrap();
    if let Some(app_state) = app_state {
        index
            .execute(
                "INSERT INTO app_settings (key, value, updated_at) VALUES ('app_state_v2', ?1, ?2)",
                params![app_state.to_string(), T],
            )
            .unwrap();
    }
    let content = (!inline).then(|| {
        let content_path = dir.join("rss-reader-content.db");
        let conn = Connection::open(&content_path).unwrap();
        conn.pragma_update(None, "journal_mode", "WAL").unwrap();
        migrate_like_sqlx(&conn, &[CONTENT], endings);
        for (entry_id, feed_id) in [(1, 1), (2, 1), (99, 1)] {
            conn.execute(
                "INSERT INTO entry_contents (entry_id, feed_id, content_html, content_hash, updated_at)
                 VALUES (?1, ?2, ?3, 'hash', ?4)",
                params![entry_id, feed_id, format!("<p>body {entry_id}</p>"), T],
            )
            .unwrap();
        }
        content_path
    });
    V01Library { index: index_path, content }
}

fn current_settings() -> Value {
    serde_json::to_value(UserSettings {
        custom_css: "body { color: red }".into(),
        ..UserSettings::default()
    })
    .unwrap()
}

async fn assert_imported_library(target: &Path, expect_generation: i64) -> Library {
    let library = Library::over(LibraryStore::open(target).await.expect("imported library opens"));
    let feeds = library.feeds.list_feeds().await.unwrap();
    assert_eq!(feeds.len(), 1, "removed feeds stay removed");
    let feed = &feeds[0];
    assert_eq!(feed.id, 1);
    assert_eq!(feed.title.as_deref(), Some("Feed A"));
    assert_eq!(feed.source_title.as_deref(), Some("Feed A"), "v0.1 names are source titles");
    assert_eq!(feed.folder.as_deref(), Some("News"));
    assert_eq!(feed.etag.as_deref(), Some("etag-a"));
    assert_eq!(
        library.scalar("SELECT generation FROM feeds WHERE id = 1").await,
        expect_generation
    );
    assert_eq!(library.scalar("SELECT COUNT(*) FROM feeds").await, 2);

    let search = EntryQuery { search_title: Some("école".into()), ..EntryQuery::default() };
    let found = library.entries.list_entries(&search).await.unwrap();
    assert_eq!(found.iter().map(|entry| entry.id).collect::<Vec<_>>(), [1]);
    assert!(found[0].is_read);
    assert_eq!(
        library.scalar("SELECT COUNT(*) FROM entries").await,
        3,
        "removed feed's entries kept"
    );
    let body = library.entries.get_content(2).await.unwrap().expect("body of entry 2");
    assert_eq!(body.content_html.as_deref(), Some("<p>body 2</p>"));
    assert_eq!(library.scalar("SELECT COUNT(*) FROM entry_contents").await, 2, "no orphan bodies");
    assert_eq!(
        library.scalar("SELECT COUNT(*) FROM entries WHERE has_content = 1").await,
        2,
        "has_content matches the bodies"
    );
    assert_eq!(library.scalar("SELECT seq FROM sqlite_sequence WHERE name = 'entries'").await, 4);
    let max_ticket = library
        .scalar("SELECT MAX(refresh_issued + refresh_published + refresh_diagnosed) FROM feeds")
        .await;
    assert_eq!(max_ticket, 0);
    library
}

#[tokio::test(flavor = "multi_thread")]
async fn a_split_library_from_v0_1_23_is_imported() {
    let dir = TempDir::new("v0123");
    let app_state = json!({ "last_opened_feed_id": 1 });
    let source = fixture(&dir.0, 7, Endings::Lf, &current_settings(), Some(&app_state));
    let index_before = std::fs::read(&source.index).unwrap();

    let outcome = import_v01(&source, &dir.target()).unwrap();
    assert_eq!((outcome.layout, outcome.schema_version), (Layout::Split, 7));
    assert_eq!(
        (outcome.feeds, outcome.entries, outcome.bodies, outcome.orphan_bodies),
        (2, 3, 2, 1)
    );
    assert_eq!(std::fs::read(&source.index).unwrap(), index_before, "the v0.1 file is unchanged");

    let library = assert_imported_library(&dir.target(), 3).await;
    let settings = library.settings.load().await.unwrap();
    assert_eq!(settings.custom_css, "body { color: red }");
    assert_eq!(library.app_state.load().await.unwrap().last_opened_feed_id, Some(1));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_split_library_from_v0_1_21_with_crlf_checksums_is_imported() {
    let dir = TempDir::new("v0121");
    let source = fixture(&dir.0, 5, Endings::Crlf, &current_settings(), None);
    let outcome = import_v01(&source, &dir.target()).unwrap();
    assert_eq!((outcome.layout, outcome.schema_version), (Layout::Split, 5));
    assert_imported_library(&dir.target(), 0).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_split_library_from_v0_1_7_is_imported() {
    let dir = TempDir::new("v017");
    let source = fixture(&dir.0, 2, Endings::Lf, &current_settings(), None);
    let outcome = import_v01(&source, &dir.target()).unwrap();
    assert_eq!((outcome.layout, outcome.schema_version), (Layout::Split, 2));
    assert_imported_library(&dir.target(), 0).await;
}

/// v0.1.6 kept five entry-list preferences in the settings and the last opened feed under its own
/// key; both move to app state. Documents keep their time.
#[tokio::test(flavor = "multi_thread")]
async fn an_inline_library_from_v0_1_6_is_imported_with_its_legacy_state() {
    let dir = TempDir::new("v016");
    let workspace = EntriesWorkspaceState {
        grouping_mode: EntryGroupingPreference::Source,
        show_archived: true,
        read_filter: ReadFilter::UnreadOnly,
        starred_filter: StarredFilter::StarredOnly,
        selected_feed_urls: vec!["https://a.example/feed.xml".into()],
    };
    let workspace_value = serde_json::to_value(&workspace).unwrap();
    let mut settings = current_settings();
    for (legacy, current) in [
        ("entry_grouping_mode", "grouping_mode"),
        ("show_archived_entries", "show_archived"),
        ("entry_read_filter", "read_filter"),
        ("entry_starred_filter", "starred_filter"),
        ("entry_filtered_feed_urls", "selected_feed_urls"),
    ] {
        settings[legacy] = workspace_value[current].clone();
    }
    settings.as_object_mut().unwrap().remove("entries_page_size");
    let source = fixture(&dir.0, 1, Endings::Crlf, &settings, None);
    assert!(source.content.is_none());
    Connection::open(&source.index)
        .unwrap()
        .execute(
            "INSERT INTO app_settings (key, value, updated_at) VALUES ('last_opened_feed_id', '1', ?1)",
            [T],
        )
        .unwrap();

    let outcome = import_v01(&source, &dir.target()).unwrap();
    assert_eq!((outcome.layout, outcome.schema_version), (Layout::Inline, 1));
    assert_eq!(outcome.moved_legacy_keys.len(), 6);
    assert!(outcome.moved_legacy_keys.iter().any(|key| key == "last_opened_feed_id"));
    assert_eq!((outcome.bodies, outcome.orphan_bodies), (2, 0), "title hashes are not bodies");
    let library = Library::over(LibraryStore::open(dir.target()).await.unwrap());
    let state = library.app_state.load().await.unwrap();
    assert_eq!(state.entries_workspace, workspace);
    assert_eq!(state.last_opened_feed_id, Some(1));
    assert_eq!(
        library
            .text("SELECT updated_at FROM app_settings WHERE key = 'user_settings'")
            .await
            .as_deref(),
        Some(T)
    );
    let imported = library.settings.load().await.unwrap();
    assert_eq!(imported.custom_css, "body { color: red }");
    assert_eq!(imported.entries_page_size, UserSettings::default().entries_page_size);
    let body = library.entries.get_content(1).await.unwrap().expect("inline body moved");
    assert_eq!(body.content_html.as_deref(), Some("<p>body 1</p>"));
    assert_eq!(library.scalar("SELECT COUNT(*) FROM entry_contents").await, 2);
    assert_eq!(library.scalar("SELECT COUNT(*) FROM entries WHERE has_content = 1").await, 2);
}

/// App state written by v0.1.7+ wins over a stale v0.1.6 key that was never removed.
#[tokio::test(flavor = "multi_thread")]
async fn app_state_wins_over_the_legacy_last_opened_key() {
    let dir = TempDir::new("stale-key");
    let app_state = json!({ "last_opened_feed_id": 1 });
    let source = fixture(&dir.0, 7, Endings::Lf, &current_settings(), Some(&app_state));
    Connection::open(&source.index)
        .unwrap()
        .execute(
            "INSERT INTO app_settings (key, value, updated_at) VALUES ('last_opened_feed_id', '2', ?1)",
            [T],
        )
        .unwrap();
    let outcome = import_v01(&source, &dir.target()).unwrap();
    assert!(outcome.moved_legacy_keys.is_empty());
    let library = Library::over(LibraryStore::open(dir.target()).await.unwrap());
    assert_eq!(library.app_state.load().await.unwrap().last_opened_feed_id, Some(1));
    assert_eq!(
        library
            .text("SELECT updated_at FROM app_settings WHERE key = 'app_state_v2'")
            .await
            .as_deref(),
        Some(T)
    );
}

/// A split library whose body database is missing imports only when no entry had a body.
#[tokio::test(flavor = "multi_thread")]
async fn a_missing_body_database_is_refused_when_bodies_existed() {
    let dir = TempDir::new("no-content");
    let mut source = fixture(&dir.0, 7, Endings::Lf, &current_settings(), None);
    source.content = None;
    refused(&source, &dir.target(), "找不到正文库");

    Connection::open(&source.index)
        .unwrap()
        .execute("UPDATE entries SET has_content = 0", [])
        .unwrap();
    let outcome = import_v01(&source, &dir.target()).unwrap();
    assert_eq!((outcome.entries, outcome.bodies, outcome.orphan_bodies), (3, 0, 0));
    let library = Library::over(LibraryStore::open(dir.target()).await.unwrap());
    assert_eq!(library.scalar("SELECT COUNT(*) FROM entry_contents").await, 0);
}

/// Two imports into one target at once: one publishes, the other finds the target taken; the
/// published library is the complete one.
#[test]
fn concurrent_imports_publish_one_complete_library() {
    let dir = TempDir::new("concurrent-import");
    let source = fixture(&dir.0, 7, Endings::Lf, &current_settings(), None);
    let target = dir.target();
    let results: Vec<_> = within(std::time::Duration::from_secs(120), move || {
        std::thread::scope(|scope| {
            let workers: Vec<_> =
                (0..4).map(|_| scope.spawn(|| import_v01(&source, &target))).collect();
            workers.into_iter().map(|worker| worker.join().unwrap()).collect()
        })
    });
    let published: Vec<_> = results.iter().filter_map(|result| result.as_ref().ok()).collect();
    assert_eq!(published.len(), 1, "{results:?}");
    assert_eq!(published[0].entries, 3);
    for error in results.iter().filter_map(|result| result.as_ref().err()) {
        assert!(matches!(error, rssr_infra::store::StoreError::LibraryExists(_)), "{error}");
    }
    assert_eq!(scratch_files(&dir.0), Vec::<String>::new());
}

/// The library the concurrent imports published is the complete one, read back from the file.
#[tokio::test(flavor = "multi_thread")]
async fn the_library_published_by_concurrent_imports_is_complete() {
    let dir = TempDir::new("concurrent-import-content");
    let source = fixture(&dir.0, 7, Endings::Lf, &current_settings(), None);
    let target = dir.target();
    let importing = target.clone();
    within(std::time::Duration::from_secs(120), move || {
        std::thread::scope(|scope| {
            for _ in 0..4 {
                scope.spawn(|| {
                    let _ = import_v01(&source, &importing);
                });
            }
        })
    });
    assert_imported_library(&target, 3).await;
}

/// What a split library holds inconsistently is reported: a body with neither HTML nor text is not
/// imported, an entry marked as having a body without one is counted, and a body filed under
/// another subscription stays with its entry.
#[tokio::test(flavor = "multi_thread")]
async fn inconsistent_split_libraries_are_imported_and_reported() {
    let dir = TempDir::new("inconsistent");
    let source = fixture(&dir.0, 7, Endings::Lf, &current_settings(), None);
    {
        let content = Connection::open(source.content.as_ref().unwrap()).unwrap();
        content
            .execute_batch(
                "UPDATE entry_contents SET content_html = NULL, content_text = NULL
                 WHERE entry_id = 2;
                 UPDATE entry_contents SET feed_id = 2 WHERE entry_id IN (1, 2);",
            )
            .unwrap();
        Connection::open(&source.index)
            .unwrap()
            .execute("UPDATE entries SET has_content = 1 WHERE id = 3", [])
            .unwrap();
    }
    let outcome = import_v01(&source, &dir.target()).unwrap();
    assert_eq!(
        (outcome.bodies, outcome.empty_bodies, outcome.missing_bodies, outcome.misfiled_bodies),
        (1, 1, 2, 1),
        "{outcome:?}"
    );
    let library = Library::over(LibraryStore::open(dir.target()).await.unwrap());
    assert_eq!(library.scalar("SELECT COUNT(*) FROM entry_contents").await, 1);
    assert_eq!(library.scalar("SELECT has_content FROM entries WHERE id = 2").await, 0);
    let body = library.entries.get_content(1).await.unwrap().expect("kept with its entry");
    assert_eq!(body.content_html.as_deref(), Some("<p>body 1</p>"));
}

/// Data still in the WAL of an open v0.1 process is part of the snapshot.
#[tokio::test(flavor = "multi_thread")]
async fn uncheckpointed_writes_are_imported() {
    let dir = TempDir::new("wal");
    let source = fixture(&dir.0, 7, Endings::Lf, &current_settings(), None);
    let writer = Connection::open(&source.index).unwrap();
    writer.pragma_update(None, "wal_autocheckpoint", 0).unwrap();
    writer
        .execute_batch(&format!(
            "INSERT INTO feeds (id, url, title, created_at, updated_at)
             VALUES (3, 'https://c.example/feed.xml', 'Feed C', '{T}', '{T}')"
        ))
        .unwrap();
    let outcome = import_v01(&source, &dir.target()).unwrap();
    assert_eq!(outcome.feeds, 3);
    drop(writer);
}

/// Runs `body` where a hang cannot hold the test.
fn within<R: Send + 'static>(
    limit: std::time::Duration,
    body: impl FnOnce() -> R + Send + 'static,
) -> R {
    let (answer, receive) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = answer.send(body());
    });
    receive.recv_timeout(limit).expect("finished within the limit")
}

/// Files next to the library other than the library and its creation lock.
fn scratch_files(dir: &Path) -> Vec<String> {
    std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("library.sqlite3."))
        .filter(|name| name != "library.sqlite3.create-lock")
        .collect()
}

fn refused(source: &V01Library, target: &Path, needle: &str) {
    let error = import_v01(source, target).expect_err("import refused").to_string();
    assert!(error.contains(needle), "{error}");
    assert!(!target.exists(), "nothing published");
    let leftovers = scratch_files(target.parent().unwrap());
    assert!(leftovers.is_empty(), "scratch files removed: {leftovers:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn development_schemas_and_altered_histories_are_refused() {
    for (label, version) in [("dev8", 8), ("dev9", 9)] {
        let dir = TempDir::new(label);
        let source = fixture(&dir.0, version, Endings::Lf, &current_settings(), None);
        refused(&source, &dir.target(), "未发布的迁移");
    }

    let dir = TempDir::new("tampered");
    let source = fixture(&dir.0, 5, Endings::Lf, &current_settings(), None);
    Connection::open(&source.index)
        .unwrap()
        .execute("UPDATE _sqlx_migrations SET checksum = x'00' WHERE version = 3", [])
        .unwrap();
    refused(&source, &dir.target(), "与发布版本不一致");

    let dir = TempDir::new("failed");
    let source = fixture(&dir.0, 5, Endings::Lf, &current_settings(), None);
    Connection::open(&source.index)
        .unwrap()
        .execute("UPDATE _sqlx_migrations SET success = FALSE WHERE version = 5", [])
        .unwrap();
    refused(&source, &dir.target(), "未完成");

    let dir = TempDir::new("unknown-settings");
    let mut settings = current_settings();
    settings["no_such_setting"] = json!(1);
    let source = fixture(&dir.0, 7, Endings::Lf, &settings, None);
    refused(&source, &dir.target(), "设置无法解析");
}

#[tokio::test(flavor = "multi_thread")]
async fn an_existing_target_is_never_overwritten() {
    let dir = TempDir::new("exists");
    let source = fixture(&dir.0, 7, Endings::Lf, &current_settings(), None);
    std::fs::write(dir.target(), b"keep me").unwrap();
    assert!(import_v01(&source, &dir.target()).is_err());
    assert_eq!(std::fs::read(dir.target()).unwrap(), b"keep me");
}

/// Unreadable app state is not imported; everything else is.
#[tokio::test(flavor = "multi_thread")]
async fn unreadable_app_state_is_left_behind() {
    let dir = TempDir::new("bad-state");
    let source =
        fixture(&dir.0, 7, Endings::Lf, &current_settings(), Some(&json!({ "future": 1 })));
    let outcome = import_v01(&source, &dir.target()).unwrap();
    assert!(outcome.dropped_app_state);
    let library = Library::over(LibraryStore::open(dir.target()).await.unwrap());
    assert_eq!(library.app_state.load().await.unwrap(), Default::default());
}

#[test]
fn a_data_directory_without_a_v01_index_has_nothing_to_import() {
    let dir = TempDir::new("empty");
    assert!(V01Library::find(&dir.0).unwrap().is_none());
    std::fs::write(dir.0.join("rss-reader.db"), b"").unwrap();
    let found = V01Library::find(&dir.0).unwrap().unwrap();
    assert!(found.content.is_none());

    // Something at the old name that is not a file is not mistaken for "no old library".
    let odd = TempDir::new("odd");
    std::fs::create_dir(odd.0.join("rss-reader.db")).unwrap();
    assert!(V01Library::find(&odd.0).is_err());

    // A link to nowhere at the old name is an error, not "no old library".
    #[cfg(unix)]
    {
        let dangling = TempDir::new("dangling");
        std::os::unix::fs::symlink(dangling.0.join("gone.db"), dangling.0.join("rss-reader.db"))
            .unwrap();
        assert!(V01Library::find(&dangling.0).is_err());
    }
}

/// Opening a data directory that holds only a v0.1 library imports it once; later opens use the
/// library.
#[tokio::test(flavor = "multi_thread")]
async fn opening_a_data_directory_imports_its_v01_library_once() {
    use rssr_infra::store::local::open_library_in;
    let dir = TempDir::new("open-imports");
    fixture(&dir.0, 7, Endings::Lf, &current_settings(), None);
    let first = open_library_in(&dir.0, Default::default()).await.unwrap();
    assert_eq!(first.imported.as_ref().map(|outcome| outcome.feeds), Some(2));
    assert_eq!(first.path, dir.target());
    drop(first);
    let second = open_library_in(&dir.0, Default::default()).await.unwrap();
    assert!(second.imported.is_none());
    assert_imported_library(&dir.target(), 3).await;
}

/// A v0.1 library that cannot be imported fails the open and leaves no empty library that would
/// hide the old data.
#[tokio::test(flavor = "multi_thread")]
async fn a_failed_import_does_not_create_an_empty_library() {
    use rssr_infra::store::local::open_library_in;
    let dir = TempDir::new("open-fails");
    fixture(&dir.0, 9, Endings::Lf, &current_settings(), None);
    assert!(open_library_in(&dir.0, Default::default()).await.is_err());
    assert!(!dir.target().exists());
}

/// Two processes starting together import once and both open the published library.
#[tokio::test(flavor = "multi_thread")]
async fn concurrent_first_opens_import_once() {
    use rssr_infra::store::local::open_library_in;
    let dir = TempDir::new("open-concurrent");
    fixture(&dir.0, 7, Endings::Lf, &current_settings(), None);
    let (a, b) = tokio::join!(
        open_library_in(&dir.0, Default::default()),
        open_library_in(&dir.0, Default::default())
    );
    let (a, b) = (a.unwrap(), b.unwrap());
    assert_eq!(usize::from(a.imported.is_some()) + usize::from(b.imported.is_some()), 1);
    let leftovers = scratch_files(&dir.0);
    assert!(leftovers.iter().all(|name| name.ends_with("-wal") || name.ends_with("-shm")));
    drop((a, b));
    assert_imported_library(&dir.target(), 3).await;
}

/// Without a v0.1 library the first open creates a new, empty library.
#[tokio::test(flavor = "multi_thread")]
async fn a_fresh_data_directory_gets_a_new_library() {
    use rssr_infra::store::local::open_library_in;
    let dir = TempDir::new("open-fresh");
    let opened = open_library_in(&dir.0.join("nested"), Default::default()).await.unwrap();
    assert!(opened.imported.is_none());
    assert!(dir.0.join("nested").join("library.sqlite3").is_file());
}
