#![cfg(not(target_arch = "wasm32"))]

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[path = "support/disk_full.rs"]
mod disk_full;
#[path = "support/library.rs"]
mod library;
#[path = "support/v01_schema.rs"]
mod v01_schema;

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
        queue_capacity: 256,
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

fn side_file(library: &std::path::Path, suffix: &str) -> PathBuf {
    let mut name = library.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}

/// Closing, through any handle, lets the running task and the one queued behind it finish, refuses
/// tasks submitted afterwards, and returns only once every connection is closed: the last one has
/// checkpointed the log into the main file and removed it, so a process that exits right after
/// leaves a complete library.
#[tokio::test(flavor = "multi_thread")]
async fn closing_finishes_queued_tasks_and_waits_for_every_connection() {
    let _watchdog = Watchdog::arm(Duration::from_secs(60));
    let dir = TempDir::new("close");
    // The queued task must not reach its start time while close waits.
    let options = StoreOptions { admission_wait: Duration::from_secs(10), ..quick() };
    let store = LibraryStore::open_with(dir.library(), options).await.unwrap();
    insert_feed(&store, "https://a.test").await.unwrap();
    let wal = side_file(&dir.library(), "-wal");
    assert!(wal.exists(), "an open library has its log");

    let (release, occupying) = occupy_writer(&store).await;
    let behind = store.clone();
    let mut queued =
        std::pin::pin!(async move { insert_feed(&behind, "https://queued.test").await });
    submit_and_look_away(queued.as_mut());
    let closing = {
        let store = store.clone();
        tokio::spawn(async move { store.close().await })
    };
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(!closing.is_finished(), "close waits for the running task and the queued one");
    assert!(matches!(insert_feed(&store, "https://late.test").await, Err(StoreError::Closed)));
    assert!(matches!(store.read(|_| Ok(())).await, Err(StoreError::Closed)));

    release.send(()).unwrap();
    occupying.await.unwrap().expect("the running task ends normally");
    queued.await.expect("the queued task still runs");
    tokio::time::timeout(Duration::from_secs(10), closing)
        .await
        .expect("close returns once the workers have ended")
        .unwrap()
        .expect("every connection closed");
    assert!(!wal.exists(), "the log was written back and removed before close returned");
    assert!(!side_file(&dir.library(), "-shm").exists());
    store.close().await.expect("closing again");

    let reopened = LibraryStore::open(dir.library()).await.unwrap();
    assert_eq!(feed_count(&reopened).await, 2);
}

/// Async and blocking callers may close the same library at once; both must wait for readers
/// and leave a checkpointed main file even when the writer has no work left.
#[tokio::test(flavor = "multi_thread")]
async fn concurrent_closes_wait_for_running_reads() {
    let _watchdog = Watchdog::arm(Duration::from_secs(60));
    let dir = TempDir::new("concurrent-close");
    let options = StoreOptions { read_deadline: Duration::from_secs(20), ..quick() };
    let store = LibraryStore::open_with(dir.library(), options).await.unwrap();
    insert_feed(&store, "https://a.test").await.unwrap();
    let (started, signal) = mpsc::channel();
    let (release, held) = mpsc::channel();
    let reading = {
        let store = store.clone();
        tokio::spawn(async move {
            store
                .read(move |conn| {
                    let count: i64 =
                        conn.query_row("SELECT count(*) FROM feeds", [], |r| r.get(0))?;
                    started.send(()).unwrap();
                    held.recv_timeout(Duration::from_secs(10)).unwrap();
                    Ok(count)
                })
                .await
        })
    };
    wait_for(signal).await;
    let closing = {
        let store = store.clone();
        tokio::spawn(async move { store.close().await })
    };
    let blocking_close = {
        let store = store.clone();
        tokio::task::spawn_blocking(move || store.close_blocking())
    };
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(!closing.is_finished());
    assert!(!blocking_close.is_finished());
    assert!(matches!(store.read(|_| Ok(())).await, Err(StoreError::Closed)));
    assert!(matches!(insert_feed(&store, "https://late.test").await, Err(StoreError::Closed)));

    release.send(()).unwrap();
    assert_eq!(reading.await.unwrap().unwrap(), 1);
    for close in [closing, blocking_close] {
        tokio::time::timeout(Duration::from_secs(10), close).await.unwrap().unwrap().unwrap();
    }
    assert!(!side_file(&dir.library(), "-wal").exists());
    assert!(!side_file(&dir.library(), "-shm").exists());
}

/// A library its process left mid-checkpoint (a crash, a kill, a power loss while the log was
/// being written back: page 1 already counts a page the file does not have) opens, and SQLite
/// recovers it from the log; the identity probe no longer refuses it as corrupt. Without its log
/// the same file is refused as corrupt by the writer's first read, untouched and with no `-wal`
/// or `-shm` left beside it.
#[tokio::test(flavor = "multi_thread")]
async fn a_library_left_mid_checkpoint_opens_and_recovers_from_its_log() {
    let dir = TempDir::new("mid-checkpoint");
    let store = LibraryStore::open_with(dir.library(), quick()).await.unwrap();
    store
        .write(|tx| {
            for i in 0..20 {
                tx.execute(
                    "INSERT INTO feeds (url, description, created_at, updated_at)
                     VALUES (?1, ?2, 't', 't')",
                    (format!("https://f{i}.test"), "x".repeat(3000)),
                )?;
            }
            Ok(())
        })
        .await
        .unwrap();
    let log = std::fs::read(side_file(&dir.library(), "-wal")).unwrap();
    store.close().await.unwrap();
    let complete = std::fs::read(dir.library()).unwrap();
    let page = 4096;
    let short = &complete[..complete.len() - page];
    // The log is what makes the file longer: its last page is not in the main file yet.
    assert!(log.len() > page);

    std::fs::write(dir.library(), short).unwrap();
    std::fs::write(side_file(&dir.library(), "-wal"), &log).unwrap();
    let reopened = LibraryStore::open(dir.library()).await.expect("opens");
    assert_eq!(feed_count(&reopened).await, 20);
    reopened.close().await.unwrap();
    assert!(!side_file(&dir.library(), "-wal").exists(), "the log was written back");
    assert_eq!(std::fs::metadata(dir.library()).unwrap().len(), complete.len() as u64);
    let check = rusqlite::Connection::open(dir.library()).unwrap();
    let integrity: String =
        check.query_row("PRAGMA integrity_check", [], |row| row.get(0)).unwrap();
    assert_eq!(integrity, "ok");
    drop(check);

    let bare = TempDir::new("mid-checkpoint-no-log");
    std::fs::create_dir_all(bare.library().parent().unwrap()).unwrap();
    std::fs::write(bare.library(), short).unwrap();
    let refused = LibraryStore::open(bare.library()).await;
    assert!(matches!(refused, Err(StoreError::Corrupt(_))), "{:?}", refused.err());
    for suffix in ["-wal", "-shm"] {
        assert!(!side_file(&bare.library(), suffix).exists(), "{suffix}");
    }
    assert_eq!(std::fs::read(bare.library()).unwrap(), short, "the file is untouched");
}

/// A connection SQLite refuses to close (here a task leaked a statement) is reported by `close`
/// rather than passed off as a clean close.
#[tokio::test(flavor = "multi_thread")]
async fn a_connection_that_cannot_be_closed_is_reported() {
    let store = LibraryStore::open_in_memory().await.unwrap();
    store
        .write(|tx| {
            std::mem::forget(tx.prepare("SELECT 1")?);
            Ok(())
        })
        .await
        .unwrap();
    let closed = store.close().await;
    assert!(matches!(closed, Err(StoreError::Busy(_))), "{closed:?}");
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
            // Called by the statement itself, so it proves the statement executed.
            conn.create_scalar_function(
                "mark_running",
                0,
                rusqlite::functions::FunctionFlags::SQLITE_UTF8,
                move |_| {
                    marker.store(true, Ordering::SeqCst);
                    Ok(1_i64)
                },
            )?;
            Ok(conn.query_row(
                "WITH RECURSIVE n(i) AS (SELECT mark_running() UNION ALL SELECT i + 1 FROM n)
                 SELECT count(*) FROM n",
                [],
                |row| row.get::<_, i64>(0),
            )?)
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
    let ran = Arc::new(AtomicBool::new(false));
    let marker = Arc::clone(&ran);
    let queued = store
        .write(move |_| {
            marker.store(true, Ordering::SeqCst);
            Ok(())
        })
        .await;
    assert!(matches!(queued, Err(StoreError::ConnectionTimeout)), "{queued:?}");
    release.send(()).unwrap();
    occupying.await.unwrap().unwrap();
    // A later task on the same writer runs after the cancelled one would have.
    insert_feed(&store, "https://after.test").await.unwrap();
    assert!(!ran.load(Ordering::SeqCst), "the timed-out task never started");
}

/// Polls `future` once without a waker that does anything: the task is submitted, then its caller
/// stops looking.
fn submit_and_look_away<F: std::future::Future>(future: std::pin::Pin<&mut F>) {
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    assert!(future.poll(&mut context).is_pending());
}

/// Reports, when dropped, the name of the thread that dropped it.
struct DropProbe(mpsc::Sender<Option<String>>);

impl Drop for DropProbe {
    fn drop(&mut self) {
        let _ = self.0.send(std::thread::current().name().map(str::to_owned));
    }
}

/// Holds the writer until released; returns the release handle and the occupying task.
async fn occupy_writer(
    store: &LibraryStore,
) -> (mpsc::Sender<()>, tokio::task::JoinHandle<Result<(), StoreError>>) {
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
    (release, occupying)
}

/// A task the worker takes after its start time never runs, even while its caller has not looked
/// at the clock yet: the worker itself cancels it (and drops what it captured).
#[tokio::test(flavor = "multi_thread")]
async fn a_task_taken_after_its_start_time_never_runs() {
    let dir = TempDir::new("taken-late");
    let store = LibraryStore::open_with(dir.library(), quick()).await.unwrap();
    let (release, occupying) = occupy_writer(&store).await;
    let ran = Arc::new(AtomicBool::new(false));
    let marker = Arc::clone(&ran);
    let (dropped, wait_dropped) = mpsc::channel();
    let probe = DropProbe(dropped);
    let mut queued = std::pin::pin!(store.write(move |_| {
        let _probe = &probe;
        marker.store(true, Ordering::SeqCst);
        Ok(())
    }));
    submit_and_look_away(queued.as_mut());
    tokio::time::sleep(Duration::from_millis(400)).await;
    release.send(()).unwrap();
    occupying.await.unwrap().unwrap();

    let dropper = wait_dropped.recv_timeout(Duration::from_secs(10)).expect("closure dropped");
    assert_eq!(dropper.as_deref(), Some("rssr-library"), "the worker cancelled it");
    assert!(matches!(queued.await, Err(StoreError::ConnectionTimeout)));
    assert!(!ran.load(Ordering::SeqCst));
    insert_feed(&store, "https://after-late.test").await.unwrap();
}

/// A panic while dropping what a cancelled task captured does not stop the worker.
#[tokio::test(flavor = "multi_thread")]
async fn a_panic_while_dropping_a_cancelled_task_is_contained() {
    struct PanicOnDrop;
    impl Drop for PanicOnDrop {
        fn drop(&mut self) {
            panic!("dropping a cancelled task's data");
        }
    }

    let dir = TempDir::new("panic-drop");
    let store = LibraryStore::open_with(dir.library(), quick()).await.unwrap();
    let (release, occupying) = occupy_writer(&store).await;
    let doomed = PanicOnDrop;
    let mut queued = std::pin::pin!(store.write(move |_| {
        let _doomed = &doomed;
        Ok(())
    }));
    submit_and_look_away(queued.as_mut());
    tokio::time::sleep(Duration::from_millis(400)).await;
    release.send(()).unwrap();
    occupying.await.unwrap().unwrap();
    assert!(matches!(queued.await, Err(StoreError::ConnectionTimeout)));
    // The same writer still serves tasks.
    insert_feed(&store, "https://after-panic.test").await.unwrap();

    // The caller's own cancellation contains it too.
    let (release, occupying) = occupy_writer(&store).await;
    let doomed = PanicOnDrop;
    let cancelled = store
        .write(move |_| {
            let _doomed = &doomed;
            Ok(())
        })
        .await;
    assert!(matches!(cancelled, Err(StoreError::ConnectionTimeout)));
    release.send(()).unwrap();
    occupying.await.unwrap().unwrap();
    insert_feed(&store, "https://after-caller-panic.test").await.unwrap();
}

/// A caller that gives up while its task waits for room in a full queue drops nothing that
/// panics into its own code, and the store keeps working.
#[tokio::test(flavor = "multi_thread")]
async fn a_task_given_up_while_waiting_for_room_is_discarded_safely() {
    struct PanicOnDrop;
    impl Drop for PanicOnDrop {
        fn drop(&mut self) {
            panic!("dropping data of a task that never got room");
        }
    }

    let dir = TempDir::new("room-panic");
    let options =
        StoreOptions { queue_capacity: 1, admission_wait: Duration::from_secs(5), ..quick() };
    let store = LibraryStore::open_with(dir.library(), options).await.unwrap();
    let (release, occupying) = occupy_writer(&store).await;
    let mut queued = std::pin::pin!(store.write(|_| Ok(())));
    submit_and_look_away(queued.as_mut());
    let doomed = PanicOnDrop;
    let waiting = store.write(move |_| {
        let _doomed = &doomed;
        Ok(())
    });
    // No room: the write is still waiting when its caller gives up.
    assert!(tokio::time::timeout(Duration::from_millis(200), waiting).await.is_err());
    release.send(()).unwrap();
    occupying.await.unwrap().unwrap();
    queued.await.unwrap();
    insert_feed(&store, "https://after-room.test").await.unwrap();
}

/// An open given up while another creator holds the creation lock does not go on to create the
/// library once the lock is free.
#[tokio::test(flavor = "multi_thread")]
async fn an_open_given_up_while_waiting_to_create_creates_nothing() {
    let dir = TempDir::new("open-gave-up");
    let path = dir.library();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let other_creator = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path.with_file_name("library.sqlite3.create-lock"))
        .unwrap();
    other_creator.lock().unwrap();
    let opening = LibraryStore::open(&path);
    assert!(tokio::time::timeout(Duration::from_millis(300), opening).await.is_err());
    other_creator.unlock().unwrap();
    // Time enough for the abandoned opener to take the free lock and create, had it continued.
    tokio::time::sleep(Duration::from_secs(1)).await;
    assert!(!path.exists(), "nothing is created for a caller that gave up");
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
    // The worker closes the broken connection after the failed task has replied, so the next
    // task's admission wait includes that close, which can exceed `quick()`'s under load.
    let options = StoreOptions { admission_wait: Duration::from_secs(5), ..quick() };
    let store = LibraryStore::open_with(dir.library(), options).await.unwrap();
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
    let untouched = std::fs::read(&path).unwrap();
    let refused = LibraryStore::open(&path).await;
    assert!(matches!(refused, Err(StoreError::NotALibrary)), "{:?}", refused.err());
    assert_eq!(listing(), ["other.db"], "no -wal or -shm was created");
    assert_eq!(std::fs::read(&path).unwrap(), untouched);
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

/// A header with no application id and no schema cookie does not make a file empty: a foreign
/// database with tables but `schema_version` set back to 0 is refused untouched.
#[tokio::test(flavor = "multi_thread")]
async fn a_foreign_database_with_a_zero_header_is_refused_untouched() {
    let dir = TempDir::new("zero-header");
    let path = dir.0.join("other.db");
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE notes (id INTEGER PRIMARY KEY); PRAGMA writable_schema = ON;
             PRAGMA schema_version = 0; PRAGMA writable_schema = OFF;",
        )
        .unwrap();
        let cookie: i64 = conn.query_row("PRAGMA schema_version", [], |row| row.get(0)).unwrap();
        assert_eq!(cookie, 0);
    }
    let before = std::fs::read(&path).unwrap();
    let refused = LibraryStore::open(&path).await;
    assert!(matches!(refused, Err(StoreError::NotALibrary)), "{:?}", refused.err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

/// An empty file where the library should be is not taken over.
#[tokio::test(flavor = "multi_thread")]
async fn an_empty_file_at_the_library_path_is_refused() {
    let dir = TempDir::new("empty-file");
    std::fs::write(dir.0.join("library.sqlite3"), b"").unwrap();
    let refused = LibraryStore::open(dir.0.join("library.sqlite3")).await;
    assert!(matches!(refused, Err(StoreError::NotALibrary)), "{:?}", refused.err());
    assert_eq!(std::fs::read(dir.0.join("library.sqlite3")).unwrap(), b"");
}

/// Characters that mean something in a `file:` URI do not confuse which file is checked.
#[tokio::test(flavor = "multi_thread")]
async fn a_library_path_with_uri_characters_opens_the_named_file() {
    let dir = TempDir::new("uri # % 中文");
    // `?` is not allowed in Windows file names; `#`, `%`, `&` and `=` are.
    let path = dir.0.join("lib #1&mode=memory %20.sqlite3");
    let store = LibraryStore::open(&path).await.unwrap();
    insert_feed(&store, "https://a.test").await.unwrap();
    drop(store);
    assert!(path.is_file(), "the library is the named file");
    let reopened = LibraryStore::open(&path).await.unwrap();
    assert_eq!(feed_count(&reopened).await, 1);
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
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        let mode: String =
            conn.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0)).unwrap();
        assert_eq!(mode, "wal", "v0.1.x databases ran in WAL mode");
        v01_schema::apply_index(&conn, 7);
    }
    let before = std::fs::read(&path).unwrap();

    let refused = LibraryStore::open(&path).await;
    assert!(matches!(refused, Err(StoreError::NotALibrary)), "{:?}", refused.err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

/// SQLite's byte range for shared locks on the main database file (`SHARED_FIRST`,
/// `SHARED_SIZE` in sqlite3.c).
#[cfg(unix)]
const SHARED_FIRST: i64 = 0x4000_0000 + 2;
#[cfg(unix)]
const SHARED_SIZE: i64 = 510;

/// Child process for `opening_again_keeps_this_process_file_locks`: reports whether another
/// process holds a lock on the shared-lock range of `RSSR_LOCK_PROBE_PATH`.
#[cfg(unix)]
#[test]
#[ignore = "child process of opening_again_keeps_this_process_file_locks"]
fn lock_state_child() {
    use std::os::fd::AsRawFd;

    let Some(path) = std::env::var_os("RSSR_LOCK_PROBE_PATH") else { return };
    let file = std::fs::OpenOptions::new().read(true).write(true).open(path).unwrap();
    let mut lock: libc::flock = unsafe { std::mem::zeroed() };
    lock.l_type = libc::F_WRLCK as libc::c_short;
    lock.l_whence = libc::SEEK_SET as libc::c_short;
    lock.l_start = SHARED_FIRST as libc::off_t;
    lock.l_len = SHARED_SIZE as libc::off_t;
    // SAFETY: a valid descriptor and a valid, initialised flock.
    let result = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETLK, &mut lock) };
    assert_eq!(result, 0);
    std::process::exit(if libc::c_int::from(lock.l_type) == libc::F_UNLCK { 10 } else { 20 });
}

#[cfg(unix)]
fn another_process_sees_a_lock(path: &std::path::Path) -> bool {
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["lock_state_child", "--exact", "--ignored", "--nocapture"])
        .env("RSSR_LOCK_PROBE_PATH", path)
        .status()
        .unwrap();
    match status.code() {
        Some(20) => true,
        Some(10) => false,
        other => panic!("lock probe failed: {other:?}"),
    }
}

/// Opening a library again in the same process must not drop the POSIX locks the existing
/// connections hold (closing any descriptor of the file would, so the identity check goes
/// through SQLite).
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn opening_again_keeps_this_process_file_locks() {
    let dir = TempDir::new("posix-locks");
    let store = LibraryStore::open(dir.library()).await.unwrap();
    assert!(another_process_sees_a_lock(&dir.library()), "an open library holds a shared lock");
    let again = LibraryStore::open(dir.library()).await.unwrap();
    assert!(another_process_sees_a_lock(&dir.library()), "opening again dropped the locks");
    drop((store, again));
}

/// The project's SQLite baseline is 3.51.3: earlier versions (except the backports 3.44.6 and
/// 3.50.7) have the WAL-reset bug, which can corrupt a database when two connections to the same
/// file write or checkpoint at the same moment (https://sqlite.org/wal.html#walreset). The GUI,
/// the CLI and the server open several connections to one library.
#[tokio::test(flavor = "multi_thread")]
async fn bundled_sqlite_has_the_wal_reset_fix() {
    let store = LibraryStore::open_in_memory().await.unwrap();
    let (version, source_id): (String, String) = store
        .read(|conn| {
            Ok(conn.query_row("SELECT sqlite_version(), sqlite_source_id()", [], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?)
        })
        .await
        .unwrap();
    let parts: Vec<u32> = version.split('.').map(|part| part.parse().unwrap()).collect();
    assert!(parts >= vec![3, 51, 3], "SQLite {version} is below the project baseline 3.51.3");
    println!("sqlite_version={version} sqlite_source_id={source_id}");
    store.close().await.unwrap();
}

/// A real full disk (phase A gate): a refresh that cannot be written fails as a storage error and
/// is rolled back whole, and a new one cannot start; the library stays sound, takes writes again once space is freed, and
/// opens again. Runs only on a small file system (`RSSR_DISK_FULL_DIR`).
#[tokio::test(flavor = "multi_thread")]
async fn writes_on_a_full_disk_roll_back_and_resume() {
    use rssr_application::{
        ErrorKind, FeedRefreshUpdate, ParsedEntryData, ParsedFeedUpdate, RefreshCommit,
        RefreshStorePort, classify,
    };

    let Some(root) = disk_full::dir("store") else { return };
    let path = root.join("library.sqlite3");
    let entries = |from: usize, count: usize| -> Vec<ParsedEntryData> {
        (from..from + count)
            .map(|index| ParsedEntryData {
                external_id: format!("entry-{index}"),
                dedup_key: format!("entry-{index}"),
                url: None,
                title: format!("Entry {index}"),
                author: None,
                summary: None,
                content_html: Some(format!("<p>{}</p>", "x".repeat(64 * 1024))),
                content_text: None,
                published_at: None,
                updated_at_source: None,
            })
            .collect()
    };
    let commit = |entries: Vec<ParsedEntryData>| RefreshCommit::Updated {
        update: FeedRefreshUpdate {
            metadata: Default::default(),
            feed: ParsedFeedUpdate { title: None, site_url: None, description: None, entries },
        },
    };
    let count = "SELECT COUNT(*) FROM entries";
    let integrity = "PRAGMA integrity_check";

    let library = library::Library::over(LibraryStore::open(&path).await.unwrap());
    let feed = library.add_feed("https://example.com/full.xml").await;
    assert_eq!(library.seed(feed.id, entries(0, 4)).await, 4);

    // A refresh under way when the disk fills: its commit fails.
    let target = library.refresh.get_target(feed.id).await.unwrap().unwrap();
    let filler = disk_full::fill(&root, 0);
    let error = library
        .refresh
        .commit(&target, commit(entries(4, 32)))
        .await
        .expect_err("the disk is full");
    assert_eq!(classify(&error).kind, ErrorKind::Storage, "{error:#}");
    // A new refresh cannot start either: taking its ticket is a write.
    let error = library.refresh.get_target(feed.id).await.expect_err("the disk is full");
    assert_eq!(classify(&error).kind, ErrorKind::Storage, "{error:#}");
    assert_eq!(library.scalar(count).await, 4, "the failed refresh left nothing");
    assert_eq!(library.text(integrity).await.as_deref(), Some("ok"));

    std::fs::remove_file(&filler).unwrap();
    assert_eq!(library.seed(feed.id, entries(4, 32)).await, 32, "writes work again");
    drop(library);
    let reopened = library::Library::over(LibraryStore::open(&path).await.unwrap());
    assert_eq!(reopened.scalar(count).await, 36);
    assert_eq!(reopened.text(integrity).await.as_deref(), Some("ok"));
}
