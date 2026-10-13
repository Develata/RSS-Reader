#![cfg(not(target_arch = "wasm32"))]

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
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

/// Aborts the test process if a test that runs an unbounded query hangs (a regression of the
/// deadline would otherwise keep the runtime waiting for the blocking thread forever).
struct Watchdog(Arc<AtomicBool>);

impl Watchdog {
    fn arm(limit: Duration) -> Self {
        let done = Arc::new(AtomicBool::new(false));
        let observed = Arc::clone(&done);
        std::thread::spawn(move || {
            let started = Instant::now();
            while started.elapsed() < limit {
                if observed.load(Ordering::Relaxed) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            eprintln!("watchdog: test exceeded {limit:?}");
            std::process::abort();
        });
        Self(done)
    }
}

impl Drop for Watchdog {
    fn drop(&mut self) {
        // Stay armed while unwinding: a failed assertion can still leave the runtime waiting
        // for a task that never ends.
        if !std::thread::panicking() {
            self.0.store(true, Ordering::Relaxed);
        }
    }
}

/// Waits (off the async runtime) for a signal from inside a store task.
async fn wait_for(signal: mpsc::Receiver<()>) {
    tokio::task::spawn_blocking(move || signal.recv_timeout(Duration::from_secs(10)))
        .await
        .unwrap()
        .expect("task signalled");
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
    let _watchdog = Watchdog::arm(Duration::from_secs(60));
    let dir = TempDir::new("deadline");
    let options = StoreOptions { readers: 1, read_deadline: Duration::from_millis(100), ..quick() };
    let store = LibraryStore::open_with(dir.library(), options).await.unwrap();
    let ran = Arc::new(AtomicBool::new(false));
    let marker = Arc::clone(&ran);
    let started = Instant::now();
    let stopped = store
        .read(move |conn| {
            marker.store(true, Ordering::SeqCst);
            Ok(conn.query_row(RUN_FOREVER, [], |row| row.get::<_, i64>(0))?)
        })
        .await;
    assert!(ran.load(Ordering::SeqCst), "the statement ran");
    assert!(matches!(stopped, Err(StoreError::Interrupted)), "{stopped:?}");
    assert!(started.elapsed() < Duration::from_secs(5));
    assert_eq!(feed_count(&store).await, 0);
}

/// Waiting for another connection's write lock ends at the task's deadline, not after SQLite's
/// fixed busy timeout.
#[tokio::test(flavor = "multi_thread")]
async fn waiting_for_another_writer_ends_at_the_deadline() {
    let _watchdog = Watchdog::arm(Duration::from_secs(60));
    let dir = TempDir::new("lock-wait");
    let holder = LibraryStore::open_with(dir.library(), quick()).await.unwrap();
    let options = StoreOptions { write_deadline: Duration::from_millis(300), ..quick() };
    let waiter = LibraryStore::open_with(dir.library(), options).await.unwrap();

    let (locked, wait_locked) = mpsc::channel();
    let (release, wait_release) = mpsc::channel::<()>();
    let holding = tokio::spawn(async move {
        holder
            .write(move |tx| {
                tx.execute(
                    "INSERT INTO feeds (url, created_at, updated_at) VALUES ('https://held.test', 't', 't')",
                    [],
                )?;
                locked.send(()).unwrap();
                wait_release.recv_timeout(Duration::from_secs(10)).unwrap();
                Ok(())
            })
            .await
    });
    wait_for(wait_locked).await;
    let begun = Instant::now();
    let blocked = insert_feed(&waiter, "https://waiting.test").await;
    let waited = begun.elapsed();
    release.send(()).unwrap();
    holding.await.unwrap().unwrap();
    assert!(matches!(blocked, Err(StoreError::Busy(_))), "{blocked:?}");
    assert!(waited < Duration::from_secs(3), "waited {waited:?}");
}

/// A queued task whose caller stopped waiting never runs.
#[tokio::test(flavor = "multi_thread")]
async fn a_task_that_times_out_in_the_queue_never_runs() {
    let dir = TempDir::new("queued");
    let store = LibraryStore::open_with(dir.library(), quick()).await.unwrap();
    let (started, wait_started) = mpsc::channel();
    let (release, wait_release) = mpsc::channel::<()>();
    let busy = store.clone();
    let occupying = tokio::spawn(async move {
        busy.write(move |_| {
            started.send(()).unwrap();
            wait_release.recv_timeout(Duration::from_secs(10)).unwrap();
            Ok(())
        })
        .await
    });
    wait_for(wait_started).await;
    let queued = insert_feed(&store, "https://queued.test").await;
    assert!(matches!(queued, Err(StoreError::ConnectionTimeout)), "{queued:?}");
    release.send(()).unwrap();
    occupying.await.unwrap().unwrap();
    assert_eq!(feed_count(&store).await, 0, "the timed-out task did not run later");
}

/// A caller that goes away interrupts its running statement long before the deadline.
#[tokio::test(flavor = "multi_thread")]
async fn a_cancelled_caller_interrupts_its_running_statement() {
    let _watchdog = Watchdog::arm(Duration::from_secs(60));
    let dir = TempDir::new("cancel-interrupt");
    let options = StoreOptions {
        admission_wait: Duration::from_secs(5),
        write_deadline: Duration::from_secs(30),
        ..quick()
    };
    let store = LibraryStore::open_with(dir.library(), options).await.unwrap();
    let (started, wait_started) = mpsc::channel();
    let long = store.clone();
    let caller = tokio::spawn(async move {
        long.write(move |tx| {
            started.send(()).unwrap();
            Ok(tx.query_row(RUN_FOREVER, [], |row| row.get::<_, i64>(0))?)
        })
        .await
    });
    wait_for(wait_started).await;
    caller.abort();
    assert!(caller.await.unwrap_err().is_cancelled());

    let begun = Instant::now();
    insert_feed(&store, "https://a.test").await.expect("writer freed after the interrupt");
    assert!(begun.elapsed() < Duration::from_secs(5), "took {:?}", begun.elapsed());
}

/// Dropping the caller does not hand the writer to the next task while the first still runs,
/// and the abandoned task is rolled back instead of committed.
#[tokio::test(flavor = "multi_thread")]
async fn a_cancelled_task_keeps_the_writer_until_it_ends_and_does_not_commit() {
    let dir = TempDir::new("cancel");
    let store = LibraryStore::open_with(dir.library(), quick()).await.unwrap();
    let (started, wait_started) = mpsc::channel();
    let (release, wait_release) = mpsc::channel::<()>();
    let long = store.clone();
    let caller = tokio::spawn(async move {
        long.write(move |tx| {
            tx.execute(
                "INSERT INTO feeds (url, created_at, updated_at) VALUES ('https://late.test', 't', 't')",
                [],
            )?;
            started.send(()).unwrap();
            // Rust code the progress handler cannot interrupt.
            wait_release.recv_timeout(Duration::from_secs(10)).unwrap();
            Ok(())
        })
        .await
    });
    wait_for(wait_started).await;
    caller.abort();
    assert!(caller.await.unwrap_err().is_cancelled());

    let blocked = insert_feed(&store, "https://a.test").await;
    assert!(matches!(blocked, Err(StoreError::ConnectionTimeout)), "{blocked:?}");

    release.send(()).unwrap();
    let mut inserted = insert_feed(&store, "https://a.test").await;
    for _ in 0..50 {
        if !matches!(inserted, Err(StoreError::ConnectionTimeout)) {
            break;
        }
        inserted = insert_feed(&store, "https://a.test").await;
    }
    inserted.expect("writer free once the task ended");
    let late: i64 = store
        .read(|conn| {
            Ok(conn.query_row(
                "SELECT count(*) FROM feeds WHERE url = 'https://late.test'",
                [],
                |row| row.get(0),
            )?)
        })
        .await
        .unwrap();
    assert_eq!(late, 0, "an abandoned task must not commit");
}

/// A connection whose cleanup fails is replaced, and what its task wrote is not committed.
#[tokio::test(flavor = "multi_thread")]
async fn a_connection_that_cannot_be_cleaned_up_is_replaced() {
    use rusqlite::hooks::{AuthAction, AuthContext, Authorization, TransactionOperation};

    let dir = TempDir::new("broken");
    let store = LibraryStore::open_with(dir.library(), quick()).await.unwrap();
    let failed = store
        .write(|tx| -> Result<(), StoreError> {
            tx.execute(
                "INSERT INTO feeds (url, created_at, updated_at) VALUES ('https://lost.test', 't', 't')",
                [],
            )?;
            tx.authorizer(Some(|context: AuthContext<'_>| match context.action {
                AuthAction::Transaction { operation: TransactionOperation::Rollback } => {
                    Authorization::Deny
                }
                _ => Authorization::Allow,
            }))?;
            Err(StoreError::NotFound)
        })
        .await;
    assert!(matches!(failed, Err(StoreError::NotFound)), "{failed:?}");
    insert_feed(&store, "https://a.test").await.expect("replacement writer");
    assert_eq!(feed_count(&store).await, 1, "the broken task's insert was never committed");
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

/// A foreign SQLite file in rollback-journal mode is refused without any write: neither its
/// bytes nor its journal mode change.
#[tokio::test(flavor = "multi_thread")]
async fn a_foreign_rollback_journal_database_is_refused_byte_for_byte() {
    let dir = TempDir::new("foreign");
    let path = dir.0.join("other.db");
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        let mode: String =
            conn.query_row("PRAGMA journal_mode = DELETE", [], |row| row.get(0)).unwrap();
        assert_eq!(mode, "delete");
        conn.execute_batch(
            "CREATE TABLE notes (id INTEGER PRIMARY KEY); INSERT INTO notes DEFAULT VALUES;",
        )
        .unwrap();
    }
    let before = std::fs::read(&path).unwrap();

    let refused = LibraryStore::open(&path).await;
    assert!(matches!(refused, Err(StoreError::NotALibrary)), "{:?}", refused.err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let conn = rusqlite::Connection::open(&path).unwrap();
    let mode: String = conn.query_row("PRAGMA journal_mode", [], |row| row.get(0)).unwrap();
    assert_eq!(mode, "delete");
}

/// A foreign database in WAL mode, closed cleanly (no `-wal`/`-shm` on disk), is refused
/// without creating or changing any file.
#[tokio::test(flavor = "multi_thread")]
async fn a_foreign_wal_database_is_refused_without_touching_any_file() {
    let dir = TempDir::new("foreign-wal");
    let path = dir.0.join("other.db");
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        let mode: String =
            conn.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0)).unwrap();
        assert_eq!(mode, "wal");
        conn.execute_batch("CREATE TABLE notes (id INTEGER PRIMARY KEY)").unwrap();
    }
    let listing = || {
        let mut names: Vec<String> = std::fs::read_dir(&dir.0)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    };
    assert_eq!(listing(), ["other.db"]);
    // A stale shared-memory file left behind: SQLite would rebuild it on open and delete it on
    // close, so it shows whether anything opened the database.
    let shm = dir.0.join("other.db-shm");
    std::fs::write(&shm, vec![0x5a_u8; 32 * 1024]).unwrap();
    let files_before = listing();
    let before = std::fs::read(&path).unwrap();

    let refused = LibraryStore::open(&path).await;
    assert!(matches!(refused, Err(StoreError::NotALibrary)), "{:?}", refused.err());
    assert_eq!(listing(), files_before, "no auxiliary file was created or removed");
    assert_eq!(std::fs::read(&shm).unwrap(), vec![0x5a_u8; 32 * 1024]);
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

/// A file that is not SQLite at all is not a library either.
#[tokio::test(flavor = "multi_thread")]
async fn a_file_that_is_not_sqlite_is_refused_untouched() {
    let dir = TempDir::new("not-sqlite");
    let path = dir.0.join("notes.txt");
    std::fs::write(&path, b"plain text, not a database").unwrap();
    let refused = LibraryStore::open(&path).await;
    assert!(matches!(refused, Err(StoreError::NotALibrary)), "{:?}", refused.err());
    assert_eq!(std::fs::read(&path).unwrap(), b"plain text, not a database");
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
