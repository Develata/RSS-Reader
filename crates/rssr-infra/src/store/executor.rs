//! Native connection ownership for the library.
//!
//! Every connection belongs to one worker thread for its whole life; tasks reach it through a
//! queue. One writer worker per store, so writes from this process queue here instead of
//! busy-waiting on SQLite's lock; reader workers are `query_only` and see WAL snapshots. An
//! in-memory library has a single worker for both.
//!
//! Each task moves through `Queued -> Running -> Committing`, or to `Cancelled`:
//! - a task still queued when its caller gives up waiting (`admission_wait`) or goes away is
//!   cancelled and never runs;
//! - a running task whose caller goes away is cancelled: the progress handler interrupts its
//!   current statement and it is rolled back;
//! - `Running -> Committing` is the commit point: a task that reaches it commits even if its
//!   caller has gone away meanwhile, so a dropped caller may or may not see its write applied
//!   (as with any interrupted request).
//!
//! A running task has a deadline (`*_deadline`, from the moment it starts): waits for another
//! connection's lock end at the deadline (busy handler), and a statement still running at the
//! deadline is interrupted (progress handler). Both read the task's context from the worker
//! thread. The handlers cannot stop Rust code inside a task; repository tasks are bounded by
//! their input and the library snapshot. An expired deadline is not checked before commit on
//! purpose: a refresh that finishes a little late is stored rather than retried.
//!
//! After every task the connection is brought back to its idle state (no open transaction, the
//! role's `query_only`); a connection that cannot be is discarded, and the worker opens a new one
//! for the next task (an in-memory library cannot, and reports itself unusable).

use std::cell::RefCell;
use std::fs::File;
use std::io::Read;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rusqlite::{Connection, Transaction, TransactionBehavior};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use tokio::sync::oneshot;

use super::connection::{self, Role};
use super::error::{StoreError, StoreResult};
use super::migrate;

/// SQLite calls the progress handler every this many VM instructions.
const PROGRESS_OPS: i32 = 1_000;

#[derive(Debug, Clone)]
pub struct StoreOptions {
    /// Reader workers for a file library; an in-memory library uses its one worker.
    pub readers: usize,
    /// How long a task may wait in the queue before it starts.
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

const QUEUED: u8 = 0;
const RUNNING: u8 = 1;
const COMMITTING: u8 = 2;
const CANCELLED: u8 = 3;

fn transition(state: &AtomicU8, from: u8, to: u8) -> bool {
    state.compare_exchange(from, to, Ordering::SeqCst, Ordering::SeqCst).is_ok()
}

/// What the busy and progress handlers of the worker's connection consult.
struct TaskContext {
    state: Arc<AtomicU8>,
    until: Instant,
}

impl TaskContext {
    fn should_stop(&self) -> bool {
        self.state.load(Ordering::SeqCst) == CANCELLED || Instant::now() >= self.until
    }
}

thread_local! {
    static CURRENT: RefCell<Option<TaskContext>> = const { RefCell::new(None) };
}

/// Runs `body` with `context` visible to the handlers of this thread's connection.
fn with_context<R>(context: TaskContext, body: impl FnOnce() -> R) -> R {
    struct Clear;
    impl Drop for Clear {
        fn drop(&mut self) {
            CURRENT.with(|current| current.borrow_mut().take());
        }
    }
    CURRENT.with(|current| *current.borrow_mut() = Some(context));
    let _clear = Clear;
    body()
}

fn current_should_stop() -> bool {
    CURRENT.with(|current| current.borrow().as_ref().is_none_or(TaskContext::should_stop))
}

/// Busy handler: keeps waiting for another connection's lock until the task must stop.
fn busy_wait(attempt: i32) -> bool {
    if current_should_stop() {
        return false;
    }
    let pause = match attempt {
        0..=4 => 1_u64 << attempt,
        _ => 25,
    };
    std::thread::sleep(Duration::from_millis(pause));
    !current_should_stop()
}

/// Opens a connection with the library settings and the context-driven handlers.
fn open_connection(path: Option<&Path>, role: Role) -> StoreResult<Connection> {
    let connection = match path {
        Some(path) => Connection::open(path)?,
        None => Connection::open_in_memory()?,
    };
    connection::configure(&connection, role)?;
    connection.busy_handler(Some(busy_wait))?;
    connection.progress_handler(PROGRESS_OPS, Some(current_should_stop))?;
    Ok(connection)
}

/// Brings a connection back to its idle state; `false` when that did not fully succeed.
fn reset(connection: &Connection, role: Role) -> bool {
    // A transaction dropped during unwinding already rolled back; this covers raw SQL that left
    // one open.
    if !connection.is_autocommit() {
        let _ = connection.execute_batch("ROLLBACK");
    }
    connection.is_autocommit()
        && connection.pragma_update(None, "query_only", role == Role::Reader).is_ok()
}

/// The connection a worker owns, reopened on demand after it was discarded.
struct Slot {
    connection: Option<Connection>,
    path: Option<PathBuf>,
    role: Role,
}

impl Slot {
    fn connection(&mut self) -> StoreResult<&mut Connection> {
        if self.connection.is_none() {
            let path = self.path.as_deref().ok_or(StoreError::TaskFailed)?;
            self.connection = Some(open_connection(Some(path), self.role)?);
        }
        self.connection.as_mut().ok_or(StoreError::TaskFailed)
    }

    fn reset_after_task(&mut self) {
        let healthy =
            self.connection.as_ref().is_some_and(|connection| reset(connection, self.role));
        if !healthy && self.connection.take().is_some() {
            tracing::warn!("资料库连接无法恢复空闲状态，已丢弃，下一个任务会重新打开");
        }
    }
}

type Job = Box<dyn FnOnce(&mut Slot) + Send>;

/// A queue served by one or more workers, each owning one connection.
struct Queue {
    sender: mpsc::Sender<Job>,
}

impl Queue {
    fn start(slots: Vec<Slot>) -> StoreResult<Self> {
        let (sender, receiver) = mpsc::channel::<Job>();
        let receiver = Arc::new(Mutex::new(receiver));
        for slot in slots {
            let receiver = Arc::clone(&receiver);
            std::thread::Builder::new()
                .name("rssr-library".to_owned())
                .spawn(move || serve(&receiver, slot))?;
        }
        Ok(Self { sender })
    }

    fn submit(&self, job: Job) -> StoreResult<()> {
        self.sender.send(job).map_err(|_| StoreError::TaskFailed)
    }
}

/// Worker loop: runs tasks one at a time until the store is dropped. A panicking task is
/// contained here; its caller sees `TaskFailed`.
fn serve(receiver: &Mutex<mpsc::Receiver<Job>>, mut slot: Slot) {
    loop {
        let job = match receiver.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).recv() {
            Ok(job) => job,
            Err(_) => return,
        };
        let _ = catch_unwind(AssertUnwindSafe(|| job(&mut slot)));
        slot.reset_after_task();
    }
}

/// Handle to one library database. Cheap to clone; the workers stop when the last handle is
/// dropped, after the queued tasks.
#[derive(Clone)]
pub struct LibraryStore {
    inner: Arc<Inner>,
}

struct Inner {
    writer: Queue,
    readers: Option<Queue>,
    journal_mode: String,
    options: StoreOptions,
}

/// Cancels the task when the caller's future is dropped before the task ended.
struct CancelOnDrop {
    state: Arc<AtomicU8>,
    armed: bool,
}

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        if self.armed && !transition(&self.state, QUEUED, CANCELLED) {
            transition(&self.state, RUNNING, CANCELLED);
        }
    }
}

/// Refuses a file that is not a library without opening it with SQLite: the identity comes from
/// the 100-byte database header, so nothing (not even a `-shm` or `-wal` file) is created or
/// changed. A missing or empty file, or a header with neither an application id nor a schema
/// change yet, is left to the migration, which decides inside its write transaction (the header
/// may lag behind changes still in the WAL).
fn check_identity(path: &Path) -> StoreResult<()> {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    let mut header = [0_u8; 100];
    let mut read = 0;
    while read < header.len() {
        match file.read(&mut header[read..])? {
            0 => break,
            count => read += count,
        }
    }
    if read == 0 {
        return Ok(());
    }
    if read < header.len() || &header[..16] != b"SQLite format 3\0" {
        return Err(StoreError::NotALibrary);
    }
    let field = |offset: usize| {
        u32::from_be_bytes([
            header[offset],
            header[offset + 1],
            header[offset + 2],
            header[offset + 3],
        ])
    };
    let application_id = field(68) as i32;
    let schema_cookie = field(40);
    if application_id == migrate::APPLICATION_ID || (application_id == 0 && schema_cookie == 0) {
        Ok(())
    } else {
        Err(StoreError::NotALibrary)
    }
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
            let mut writer = open_connection(None, Role::Writer)?;
            let context = TaskContext {
                state: Arc::new(AtomicU8::new(RUNNING)),
                until: Instant::now() + connection::BUSY_TIMEOUT,
            };
            with_context(context, || migrate::migrate(&mut writer, &now()))?;
            let writer = Queue::start(vec![Slot {
                connection: Some(writer),
                path: None,
                role: Role::Writer,
            }])?;
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
        let mut writer = open_connection(Some(path), Role::Writer)?;
        let journal_mode = prepare_file(&mut writer)?;
        let mut readers = Vec::with_capacity(options.readers);
        for _ in 0..options.readers {
            readers.push(Slot {
                connection: Some(open_connection(Some(path), Role::Reader)?),
                path: Some(path.to_path_buf()),
                role: Role::Reader,
            });
        }
        let readers = if readers.is_empty() { None } else { Some(Queue::start(readers)?) };
        let writer = Queue::start(vec![Slot {
            connection: Some(writer),
            path: Some(path.to_path_buf()),
            role: Role::Writer,
        }])?;
        Ok(Self::assemble(writer, readers, journal_mode, options))
    }

    fn assemble(
        writer: Queue,
        readers: Option<Queue>,
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
        // Reads stay read-only when they run on the writer, as they would on a reader.
        let (queue, borrows_writer) = match &self.inner.readers {
            Some(readers) => (readers, false),
            None => (&self.inner.writer, true),
        };
        let deadline = self.inner.options.read_deadline;
        self.run(queue, TransactionBehavior::Deferred, deadline, borrows_writer, move |tx| task(tx))
            .await
    }

    /// Runs `task` in a write transaction, committed if it returns `Ok` and was not cancelled
    /// before its commit point.
    pub async fn write<R, F>(&self, task: F) -> StoreResult<R>
    where
        R: Send + 'static,
        F: FnOnce(&Transaction<'_>) -> StoreResult<R> + Send + 'static,
    {
        let deadline = self.inner.options.write_deadline;
        self.run(&self.inner.writer, TransactionBehavior::Immediate, deadline, false, task).await
    }

    async fn run<R, F>(
        &self,
        queue: &Queue,
        behavior: TransactionBehavior,
        deadline: Duration,
        read_only: bool,
        task: F,
    ) -> StoreResult<R>
    where
        R: Send + 'static,
        F: FnOnce(&Transaction<'_>) -> StoreResult<R> + Send + 'static,
    {
        let state = Arc::new(AtomicU8::new(QUEUED));
        let (reply, mut receive) = oneshot::channel();
        let job_state = Arc::clone(&state);
        queue.submit(Box::new(move |slot: &mut Slot| {
            if !transition(&job_state, QUEUED, RUNNING) {
                return;
            }
            let context =
                TaskContext { state: Arc::clone(&job_state), until: Instant::now() + deadline };
            let result = with_context(context, || -> StoreResult<R> {
                let connection = slot.connection()?;
                if read_only {
                    connection.pragma_update(None, "query_only", true)?;
                }
                let tx = connection.transaction_with_behavior(behavior)?;
                let value = task(&tx)?;
                if !transition(&job_state, RUNNING, COMMITTING) {
                    return Err(StoreError::Interrupted);
                }
                tx.commit()?;
                Ok(value)
            });
            let _ = reply.send(result);
        }))?;
        let mut cancel = CancelOnDrop { state: Arc::clone(&state), armed: true };
        let result =
            match tokio::time::timeout(self.inner.options.admission_wait, &mut receive).await {
                Ok(result) => result,
                Err(_) if transition(&state, QUEUED, CANCELLED) => {
                    cancel.armed = false;
                    return Err(StoreError::ConnectionTimeout);
                }
                Err(_) => receive.await,
            };
        cancel.armed = false;
        result.map_err(|_| StoreError::TaskFailed)?
    }
}

/// Switches the file to WAL and migrates it within one budget of [`connection::BUSY_TIMEOUT`],
/// retrying while another connection does the same.
///
/// Two connections preparing one new file at once (the GUI and the CLI starting together) can
/// deadlock: each holds a shared lock and wants the exclusive one to change the journal mode.
/// SQLite then answers `SQLITE_BUSY` at once instead of calling the busy handler. Both steps are
/// idempotent and the migration is one transaction, so retrying is safe; ordinary lock waits
/// inside either step end at the same deadline through the busy handler.
fn prepare_file(writer: &mut Connection) -> StoreResult<String> {
    let until = Instant::now() + connection::BUSY_TIMEOUT;
    let context = TaskContext { state: Arc::new(AtomicU8::new(RUNNING)), until };
    with_context(context, || {
        loop {
            let attempt = connection::enable_wal(writer)
                .and_then(|mode| migrate::migrate(writer, &now()).map(|()| mode));
            match attempt {
                Err(StoreError::Busy(_)) if Instant::now() < until => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                other => break other,
            }
        }
    })
}

fn now() -> String {
    OffsetDateTime::now_utc().format(&Rfc3339).unwrap_or_default()
}
