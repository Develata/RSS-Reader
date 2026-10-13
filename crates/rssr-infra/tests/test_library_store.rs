#![cfg(not(target_arch = "wasm32"))]

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use rssr_infra::store::{LibraryStore, StoreError, StoreOptions};

struct TempDir(PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("rssr-store-{label}-{nonce}"));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn library(&self) -> PathBuf {
        self.0.join("nested").join("library.sqlite3")
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn quick() -> StoreOptions {
    StoreOptions {
        readers: 2,
        admission_wait: Duration::from_millis(200),
        read_deadline: Duration::from_secs(5),
        write_deadline: Duration::from_secs(5),
    }
}

const RUN_FOREVER: &str = "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n)
                           SELECT count(*) FROM n";

async fn insert_feed(store: &LibraryStore, url: &'static str) -> Result<i64, StoreError> {
    store
        .write(move |tx| {
            tx.execute(
                "INSERT INTO feeds (url, created_at, updated_at) VALUES (?1, 't', 't')",
                [url],
            )?;
            Ok(tx.last_insert_rowid())
        })
        .await
}

async fn feed_count(store: &LibraryStore) -> i64 {
    store
        .read(|conn| Ok(conn.query_row("SELECT count(*) FROM feeds", [], |row| row.get(0))?))
        .await
        .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn file_library_is_created_in_wal_mode_and_reopens() {
    let dir = TempDir::new("open");
    let store = LibraryStore::open(dir.library()).await.unwrap();
    assert_eq!(store.journal_mode(), "wal");
    insert_feed(&store, "https://a.test").await.unwrap();
    drop(store);

    let reopened = LibraryStore::open(dir.library()).await.unwrap();
    assert_eq!(feed_count(&reopened).await, 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn write_commits_on_ok_and_rolls_back_on_error() {
    let store = LibraryStore::open_in_memory().await.unwrap();
    let failed = store
        .write(|tx| {
            tx.execute(
                "INSERT INTO feeds (url, created_at, updated_at) VALUES ('https://a.test', 't', 't')",
                [],
            )?;
            tx.execute(
                "INSERT INTO feeds (url, created_at, updated_at) VALUES ('https://a.test', 't', 't')",
                [],
            )?;
            Ok(())
        })
        .await;
    assert!(matches!(failed, Err(StoreError::Conflict(_))), "{failed:?}");
    assert_eq!(feed_count(&store).await, 0);
    insert_feed(&store, "https://a.test").await.unwrap();
    assert_eq!(feed_count(&store).await, 1);
}

/// Reads cannot write, also when an in-memory library runs them on its only connection, and
/// that connection still writes afterwards.
#[tokio::test(flavor = "multi_thread")]
async fn reads_cannot_write() {
    let dir = TempDir::new("query-only");
    let file = LibraryStore::open_with(dir.library(), quick()).await.unwrap();
    let memory = LibraryStore::open_in_memory().await.unwrap();
    for store in [&file, &memory] {
        let attempt = store
            .read(|conn| {
                conn.execute(
                    "INSERT INTO feeds (url, created_at, updated_at) VALUES ('https://a.test', 't', 't')",
                    [],
                )?;
                Ok(())
            })
            .await;
        assert!(attempt.is_err(), "a read wrote");
        assert_eq!(feed_count(store).await, 0);
        insert_feed(store, "https://a.test").await.unwrap();
        assert_eq!(feed_count(store).await, 1);
    }
}

/// All statements of one read task see one snapshot, even when a write commits in between.
#[tokio::test(flavor = "multi_thread")]
async fn a_read_task_sees_one_snapshot() {
    let dir = TempDir::new("snapshot");
    let store = LibraryStore::open_with(dir.library(), quick()).await.unwrap();
    insert_feed(&store, "https://a.test").await.unwrap();

    let (first_done, wait_first) = mpsc::channel();
    let (write_done, wait_write) = mpsc::channel::<()>();
    let reader = store.clone();
    let read = tokio::spawn(async move {
        reader
            .read(move |conn| {
                let count = |conn: &rusqlite::Connection| -> rusqlite::Result<i64> {
                    conn.query_row("SELECT count(*) FROM feeds", [], |row| row.get(0))
                };
                let before = count(conn)?;
                first_done.send(()).unwrap();
                wait_write.recv().unwrap();
                Ok((before, count(conn)?))
            })
            .await
    });
    tokio::task::spawn_blocking(move || wait_first.recv().unwrap()).await.unwrap();
    insert_feed(&store, "https://b.test").await.unwrap();
    write_done.send(()).unwrap();

    assert_eq!(read.await.unwrap().unwrap(), (1, 1));
    assert_eq!(feed_count(&store).await, 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn deadline_interrupts_a_runaway_read_and_the_connection_recovers() {
    let dir = TempDir::new("deadline");
    let options = StoreOptions { readers: 1, read_deadline: Duration::from_millis(100), ..quick() };
    let store = LibraryStore::open_with(dir.library(), options).await.unwrap();
    let started = Instant::now();
    let stopped =
        store.read(|conn| Ok(conn.query_row(RUN_FOREVER, [], |row| row.get::<_, i64>(0))?)).await;
    assert!(matches!(stopped, Err(StoreError::Interrupted)), "{stopped:?}");
    assert!(started.elapsed() < Duration::from_secs(5));
    assert_eq!(feed_count(&store).await, 0);
}

/// Dropping the caller does not hand the writer to the next task while the first still runs.
#[tokio::test(flavor = "multi_thread")]
async fn a_cancelled_caller_keeps_the_writer_until_its_task_ends() {
    let options = StoreOptions { write_deadline: Duration::from_millis(800), ..quick() };
    let dir = TempDir::new("cancel");
    let store = LibraryStore::open_with(dir.library(), options).await.unwrap();
    let started = Instant::now();
    let long = store.clone();
    let caller = tokio::spawn(async move {
        long.write(|tx| Ok(tx.query_row(RUN_FOREVER, [], |row| row.get::<_, i64>(0))?)).await
    });
    tokio::time::sleep(Duration::from_millis(50)).await;
    caller.abort();
    assert!(caller.await.unwrap_err().is_cancelled());

    let blocked = insert_feed(&store, "https://a.test").await;
    assert!(matches!(blocked, Err(StoreError::ConnectionTimeout)), "{blocked:?}");

    tokio::time::sleep(Duration::from_millis(900).saturating_sub(started.elapsed())).await;
    insert_feed(&store, "https://a.test").await.unwrap();
    assert_eq!(feed_count(&store).await, 1);
}

/// A panicking task rolls back and the connection goes back to the pool.
#[tokio::test(flavor = "multi_thread")]
async fn a_panicking_task_rolls_back_and_returns_its_connection() {
    let store = LibraryStore::open_in_memory().await.unwrap();
    let panicked = store
        .write(|tx| -> Result<(), StoreError> {
            tx.execute(
                "INSERT INTO feeds (url, created_at, updated_at) VALUES ('https://a.test', 't', 't')",
                [],
            )?;
            panic!("task failure");
        })
        .await;
    assert!(matches!(panicked, Err(StoreError::TaskFailed)), "{panicked:?}");
    assert_eq!(feed_count(&store).await, 0);
    insert_feed(&store, "https://b.test").await.unwrap();
    assert_eq!(feed_count(&store).await, 1);
}

/// Handles creating the same library at once (like the GUI and the CLI starting together) all
/// succeed and record the baseline once.
#[tokio::test(flavor = "multi_thread")]
async fn concurrent_first_opens_migrate_once() {
    let dir = TempDir::new("concurrent-open");
    let opens = (0..8).map(|_| {
        let path = dir.library();
        tokio::spawn(async move { LibraryStore::open(path).await })
    });
    let mut stores = Vec::new();
    for open in opens.collect::<Vec<_>>() {
        stores.push(open.await.unwrap().unwrap());
    }
    let history: i64 = stores[0]
        .read(|conn| {
            Ok(conn.query_row("SELECT count(*) FROM schema_migrations", [], |row| row.get(0))?)
        })
        .await
        .unwrap();
    assert_eq!(history, 1);
    insert_feed(&stores[0], "https://a.test").await.unwrap();
    for store in &stores {
        assert_eq!(store.journal_mode(), "wal");
        assert_eq!(feed_count(store).await, 1);
    }
}

/// A v0.1.x index database is not a library: opening it fails and leaves it untouched.
#[tokio::test(flavor = "multi_thread")]
async fn a_v01_database_is_refused_and_left_untouched() {
    let dir = TempDir::new("v01");
    let path = dir.0.join("rss-reader.db");
    let pool = rssr_infra::db::create_sqlite_pool(&format!("sqlite://{}?mode=rwc", path.display()))
        .await
        .unwrap();
    rssr_infra::db::migrate(&pool).await.unwrap();
    pool.close().await;
    let before = std::fs::read(&path).unwrap();

    let refused = LibraryStore::open(&path).await;
    assert!(matches!(refused, Err(StoreError::NotALibrary)), "{:?}", refused.err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
}
