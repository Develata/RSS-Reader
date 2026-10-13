//! Native connection ownership for the library.
//!
//! Invariants:
//! - one writer connection per store, so writes from this process queue here instead of
//!   busy-waiting on SQLite's lock; readers are `query_only` and see WAL snapshots;
//! - a task gets a connection only after the previous task on it has returned: the connection
//!   and the admission permit move into the blocking task and come back when it ends, even when
//!   the caller's future is dropped (`spawn_blocking` cannot be cancelled);
//! - every task runs in one transaction (`BEGIN IMMEDIATE` for writes, deferred for reads);
//! - a connection goes back to its pool only outside any transaction with its settings
//!   restored; a connection whose cleanup fails is replaced (or, for an in-memory library,
//!   retired together with its permit), also after a panic.
//!
//! Time budget (`StoreOptions::*_deadline`), counted from admission, so waiting for a blocking
//! thread counts too:
//! - waiting for SQLite's write lock (other processes) is bounded by the remaining budget;
//! - a statement still running at the deadline is interrupted by the progress handler, which
//!   also interrupts it once the caller has gone away;
//! - a task whose caller has gone away is rolled back instead of committed.
//!
//! The progress handler counts per statement, so it cannot stop a loop of short statements:
//! tasks must be bounded by their input (every repository task is). An expired budget is not
//! checked before commit on purpose: a refresh that finishes its work a little late should be
//! stored, not retried forever.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rusqlite::{Connection, OpenFlags, Transaction, TransactionBehavior};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use super::connection::{self, Role};
use super::error::{StoreError, StoreResult};
use super::migrate;

/// SQLite calls the progress handler every this many VM instructions.
const PROGRESS_OPS: i32 = 1_000;

#[derive(Debug, Clone)]
pub struct StoreOptions {
    /// Reader connections for a file library; an in-memory library uses its one connection.
    pub readers: usize,
    /// How long a task waits for a free connection.
    pub admission_wait: Duration,
    pub read_deadline: Duration,
    pub write_deadline: Duration,
}

impl Default for StoreOptions {
    fn default() -> Self {
        Self {
            readers: 4,
            admission_wait: Duration::from_secs(30),
            read_deadline: Duration::from_secs(30),
            write_deadline: Duration::from_secs(60),
        }
    }
}

/// Handle to one library database. Cheap to clone.
#[derive(Clone)]
pub struct LibraryStore {
    inner: Arc<Inner>,
}

struct Inner {
    writer: Pool,
    readers: Option<Pool>,
    journal_mode: String,
    options: StoreOptions,
}

#[derive(Clone, Copy)]
enum PoolKind {
    Writer,
    Reader,
}

impl Inner {
    fn pool(&self, kind: PoolKind) -> &Pool {
        match kind {
            PoolKind::Reader => self.readers.as_ref().unwrap_or(&self.writer),
            PoolKind::Writer => &self.writer,
        }
    }
}

/// Idle connections plus one permit per connection: holding a permit guarantees an idle one.
struct Pool {
    idle: Mutex<Vec<Connection>>,
    permits: Arc<Semaphore>,
    /// Opens a replacement for a connection that could not be cleaned up; `None` for an
    /// in-memory library, whose data lives in its only connection.
    reopen: Option<(PathBuf, Role)>,
}

impl Pool {
    fn new(connections: Vec<Connection>, reopen: Option<(PathBuf, Role)>) -> Self {
        let permits = Arc::new(Semaphore::new(connections.len()));
        Self { idle: Mutex::new(connections), permits, reopen }
    }

    fn take(&self) -> StoreResult<Connection> {
        let mut idle = self.idle.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        idle.pop().ok_or(StoreError::TaskFailed)
    }

    fn put(&self, connection: Connection) {
        self.idle.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).push(connection);
    }

    fn replacement(&self) -> Option<Connection> {
        let (path, role) = self.reopen.as_ref()?;
        match open_connection(path, *role) {
            Ok(connection) => Some(connection),
            Err(error) => {
                tracing::error!(%error, "无法重新打开资料库连接，连接池缩小");
                None
            }
        }
    }
}

/// Returns the connection to its pool when the task ends, including by panic, and releases the
/// permit only after that.
struct Lease<'a> {
    pool: &'a Pool,
    connection: Option<Connection>,
    permit: Option<OwnedSemaphorePermit>,
    /// A read borrowed the writer (in-memory library) and made it `query_only` meanwhile.
    restore_writable: bool,
}

impl Lease<'_> {
    fn connection(&mut self) -> StoreResult<&mut Connection> {
        self.connection.as_mut().ok_or(StoreError::TaskFailed)
    }
}

/// Brings a connection back to its idle state; `false` when that did not fully succeed.
fn reset(connection: &Connection, restore_writable: bool) -> bool {
    // A transaction dropped during unwinding already rolled back; this covers raw SQL that left
    // one open.
    if !connection.is_autocommit() {
        let _ = connection.execute_batch("ROLLBACK");
    }
    let mut clean = connection.is_autocommit();
    clean &= connection.progress_handler(0, None::<fn() -> bool>).is_ok();
    clean &= connection.busy_timeout(connection::BUSY_TIMEOUT).is_ok();
    if restore_writable {
        clean &= connection.pragma_update(None, "query_only", false).is_ok();
    }
    clean
}

impl Drop for Lease<'_> {
    fn drop(&mut self) {
        let returned = match self.connection.take() {
            Some(connection) if reset(&connection, self.restore_writable) => Some(connection),
            Some(broken) => {
                tracing::warn!("资料库连接清理失败，丢弃该连接");
                drop(broken);
                self.pool.replacement()
            }
            None => None,
        };
        match returned {
            Some(connection) => self.pool.put(connection),
            // Keep "a permit guarantees an idle connection": the pool shrinks by one.
            None => {
                if let Some(permit) = self.permit.take() {
                    permit.forget();
                }
            }
        }
        // Any remaining permit is released after the connection is back.
        drop(self.permit.take());
    }
}

/// Sets the task's cancellation flag when the caller's future is dropped before the task ends.
struct CancelOnDrop {
    cancelled: Arc<AtomicBool>,
    armed: bool,
}

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        if self.armed {
            self.cancelled.store(true, Ordering::Relaxed);
        }
    }
}

fn open_connection(path: &Path, role: Role) -> StoreResult<Connection> {
    let connection = Connection::open(path)?;
    connection::configure(&connection, role)?;
    Ok(connection)
}

/// Refuses a file that is not a library before anything could write to it: the identity is read
/// on a read-only connection, which can neither change the journal mode nor roll back a hot
/// journal. A missing or empty file is a new library.
fn check_identity(path: &Path) -> StoreResult<()> {
    if std::fs::metadata(path).map(|metadata| metadata.len() == 0).unwrap_or(true) {
        return Ok(());
    }
    let probe = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    probe.busy_timeout(connection::BUSY_TIMEOUT)?;
    migrate::identity(&probe).map(drop)
}

impl LibraryStore {
    /// Opens (creating if needed) the library at `path` and brings its schema up to date.
    pub async fn open(path: impl AsRef<Path>) -> StoreResult<Self> {
        Self::open_with(path, StoreOptions::default()).await
    }

    pub async fn open_with(path: impl AsRef<Path>, options: StoreOptions) -> StoreResult<Self> {
        let path = path.as_ref().to_path_buf();
        tokio::task::spawn_blocking(move || Self::open_file(&path, options))
            .await
            .map_err(|_| StoreError::TaskFailed)?
    }

    /// A private in-memory library on a single connection (tests, throwaway use).
    pub async fn open_in_memory() -> StoreResult<Self> {
        let options = StoreOptions { readers: 0, ..StoreOptions::default() };
        tokio::task::spawn_blocking(move || {
            let mut writer = Connection::open_in_memory()?;
            connection::configure(&writer, Role::Writer)?;
            migrate::migrate(&mut writer, &now())?;
            let writer = Pool::new(vec![writer], None);
            Ok(Self::assemble(writer, None, "memory".to_owned(), options))
        })
        .await
        .map_err(|_| StoreError::TaskFailed)?
    }

    fn open_file(path: &Path, options: StoreOptions) -> StoreResult<Self> {
        if let Some(parent) = path.parent().filter(|parent| !parent.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        check_identity(path)?;
        let mut writer = open_connection(path, Role::Writer)?;
        let journal_mode = prepare_file(&mut writer)?;
        let readers = (0..options.readers)
            .map(|_| open_connection(path, Role::Reader))
            .collect::<StoreResult<Vec<_>>>()?;
        let readers = (!readers.is_empty())
            .then(|| Pool::new(readers, Some((path.to_path_buf(), Role::Reader))));
        let writer = Pool::new(vec![writer], Some((path.to_path_buf(), Role::Writer)));
        Ok(Self::assemble(writer, readers, journal_mode, options))
    }

    fn assemble(
        writer: Pool,
        readers: Option<Pool>,
        journal_mode: String,
        options: StoreOptions,
    ) -> Self {
        Self { inner: Arc::new(Inner { writer, readers, journal_mode, options }) }
    }

    /// Journal mode in effect: `wal` for a file library unless the file system refused it.
    pub fn journal_mode(&self) -> &str {
        &self.inner.journal_mode
    }

    /// Runs `task` in a read transaction (one snapshot for all its statements).
    pub async fn read<R, F>(&self, task: F) -> StoreResult<R>
    where
        R: Send + 'static,
        F: FnOnce(&Connection) -> StoreResult<R> + Send + 'static,
    {
        let deadline = self.inner.options.read_deadline;
        self.run(PoolKind::Reader, TransactionBehavior::Deferred, deadline, move |tx| task(tx))
            .await
    }

    /// Runs `task` in a write transaction, committed if it returns `Ok` and its caller is still
    /// waiting.
    pub async fn write<R, F>(&self, task: F) -> StoreResult<R>
    where
        R: Send + 'static,
        F: FnOnce(&Transaction<'_>) -> StoreResult<R> + Send + 'static,
    {
        let deadline = self.inner.options.write_deadline;
        self.run(PoolKind::Writer, TransactionBehavior::Immediate, deadline, task).await
    }

    async fn run<R, F>(
        &self,
        kind: PoolKind,
        behavior: TransactionBehavior,
        deadline: Duration,
        task: F,
    ) -> StoreResult<R>
    where
        R: Send + 'static,
        F: FnOnce(&Transaction<'_>) -> StoreResult<R> + Send + 'static,
    {
        let permits = Arc::clone(&self.inner.pool(kind).permits);
        let permit =
            tokio::time::timeout(self.inner.options.admission_wait, permits.acquire_owned())
                .await
                .map_err(|_| StoreError::ConnectionTimeout)?
                .map_err(|_| StoreError::TaskFailed)?;
        let until = Instant::now() + deadline;
        let mut cancel = CancelOnDrop { cancelled: Arc::new(AtomicBool::new(false)), armed: true };
        let cancelled = Arc::clone(&cancel.cancelled);
        let inner = Arc::clone(&self.inner);
        let result = tokio::task::spawn_blocking(move || {
            let pool = inner.pool(kind);
            // Reads stay read-only when they run on the writer, as they would on a reader.
            let borrows_writer = matches!(kind, PoolKind::Reader) && inner.readers.is_none();
            let mut lease = Lease {
                pool,
                connection: None,
                permit: Some(permit),
                restore_writable: borrows_writer,
            };
            lease.connection = Some(pool.take()?);
            let connection = lease.connection()?;
            if borrows_writer {
                connection.pragma_update(None, "query_only", true)?;
            }
            run_task(connection, behavior, until, &cancelled, task)
        })
        .await
        .map_err(|_| StoreError::TaskFailed);
        cancel.armed = false;
        result?
    }
}

fn run_task<R>(
    connection: &mut Connection,
    behavior: TransactionBehavior,
    until: Instant,
    cancelled: &Arc<AtomicBool>,
    task: impl FnOnce(&Transaction<'_>) -> StoreResult<R>,
) -> StoreResult<R> {
    let remaining = until.saturating_duration_since(Instant::now());
    if remaining.is_zero() || cancelled.load(Ordering::Relaxed) {
        return Err(StoreError::Interrupted);
    }
    connection.busy_timeout(remaining.min(connection::BUSY_TIMEOUT))?;
    let stop = Arc::clone(cancelled);
    connection.progress_handler(
        PROGRESS_OPS,
        Some(move || stop.load(Ordering::Relaxed) || Instant::now() >= until),
    )?;
    let tx = connection.transaction_with_behavior(behavior)?;
    let value = task(&tx)?;
    if cancelled.load(Ordering::Relaxed) {
        return Err(StoreError::Interrupted);
    }
    tx.commit()?;
    Ok(value)
}

/// Switches the file to WAL and migrates it, retrying while another connection does the same,
/// within one shared budget of [`connection::BUSY_TIMEOUT`].
///
/// Two connections preparing one new file at once (the GUI and the CLI starting together) can
/// deadlock: each holds a shared lock and wants the exclusive one to change the journal mode.
/// SQLite then answers `SQLITE_BUSY` at once instead of waiting in the busy handler. Both steps
/// are idempotent and the migration is one transaction, so retrying is safe.
fn prepare_file(writer: &mut Connection) -> StoreResult<String> {
    let until = Instant::now() + connection::BUSY_TIMEOUT;
    let result = loop {
        let remaining = until.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break Err(StoreError::Interrupted);
        }
        writer.busy_timeout(remaining)?;
        let attempt = connection::enable_wal(writer)
            .and_then(|mode| migrate::migrate(writer, &now()).map(|()| mode));
        match attempt {
            Err(StoreError::Busy(_)) => std::thread::sleep(Duration::from_millis(20)),
            other => break other,
        }
    };
    writer.busy_timeout(connection::BUSY_TIMEOUT)?;
    result
}

fn now() -> String {
    OffsetDateTime::now_utc().format(&Rfc3339).unwrap_or_default()
}
