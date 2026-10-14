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
//! - `Running -> Committing` is where cancellation stops being accepted: from then on the commit
//!   runs to its end (it can still fail, for example when a lock wait reaches the deadline), and
//!   a caller that went away does not learn which way it went;
//! - a queued task carries the time it may start until; a task the worker marks running after
//!   that time is cancelled instead of run, and a cancelled task's closure (with the data it
//!   captured) is dropped at once, a panic in that drop contained;
//! - each queue admits at most `queue_capacity` tasks not yet taken by a worker (cancelled ones
//!   included); waiting for room counts against the admission time.
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
//!
//! Opening runs on its own thread too. The path is made absolute once, and the identity probe and
//! every connection use that one path (connections never read it as a URI). A missing library is
//! created in a scratch file under the path's creation lock and published only when complete, with
//! its application id in the header ([`files::CreationLock`]), so an existing library file either
//! is complete or is not a library at all; its identity is read with [`files::check_library`]
//! before anything could write to it. Switching to WAL and the migration share one budget of
//! [`connection::BUSY_TIMEOUT`]; checking the file and opening the connections are local file
//! operations outside it.

use std::cell::RefCell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rusqlite::{Connection, OpenFlags, Transaction, TransactionBehavior};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use tokio::sync::{OwnedSemaphorePermit, Semaphore, oneshot};

use super::connection::{self, Role};
use super::error::{StoreError, StoreResult};
use super::{files, migrate};

/// SQLite calls the progress handler every this many VM instructions.
const PROGRESS_OPS: i32 = 1_000;

#[derive(Debug, Clone)]
pub struct StoreOptions {
    /// Reader workers for a file library; an in-memory library uses its one worker.
    pub readers: usize,
    /// How long a task may wait for room in the queue and in the queue before it starts.
    pub admission_wait: Duration,
    /// Tasks a queue holds before a worker takes them; further tasks wait for room.
    pub queue_capacity: usize,
    pub read_deadline: Duration,
    pub write_deadline: Duration,
}

impl Default for StoreOptions {
    fn default() -> Self {
        Self {
            readers: 4,
            admission_wait: Duration::from_secs(30),
            queue_capacity: 256,
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

/// Opens a connection with the library settings and the context-driven handlers. A path is a
/// file name, never a URI (`file:` names stay names).
fn open_connection(path: Option<&Path>, role: Role) -> StoreResult<Connection> {
    let connection = match path {
        Some(path) => Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?,
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

/// Drops the closure of a task that will not run; a panic in dropping what it captured is
/// contained, so it can neither stop a worker nor reach a caller that gave up.
fn discard(job: Option<Job>) {
    let _ = catch_unwind(AssertUnwindSafe(move || drop(job)));
}

/// A queued task: its state, the time it may start until, its closure until it starts or is
/// cancelled, and its room in the queue until a worker takes it.
struct Pending {
    state: Arc<AtomicU8>,
    start_until: Instant,
    job: Mutex<Option<Job>>,
    room: Mutex<Option<OwnedSemaphorePermit>>,
}

impl Pending {
    fn take_job(&self) -> Option<Job> {
        self.job.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).take()
    }

    /// Cancels the task if it has not started; its closure and captured data are dropped now.
    fn cancel_if_queued(&self) -> bool {
        let cancelled = transition(&self.state, QUEUED, CANCELLED);
        if cancelled {
            discard(self.take_job());
        }
        cancelled
    }
}

/// A queue served by one or more workers, each owning one connection. It holds at most
/// `capacity` small task cells (a cancelled task's closure is dropped when it is cancelled; its
/// cell leaves when a worker takes it).
struct Queue {
    sender: mpsc::Sender<Arc<Pending>>,
    room: Arc<Semaphore>,
}

impl Queue {
    fn start(slots: Vec<Slot>, capacity: usize) -> StoreResult<Self> {
        let (sender, receiver) = mpsc::channel::<Arc<Pending>>();
        let receiver = Arc::new(Mutex::new(receiver));
        for slot in slots {
            let receiver = Arc::clone(&receiver);
            std::thread::Builder::new()
                .name("rssr-library".to_owned())
                .spawn(move || serve(&receiver, slot))?;
        }
        Ok(Self { sender, room: Arc::new(Semaphore::new(capacity.max(1))) })
    }

    /// Waits for room until the task's start time, then queues it.
    async fn submit(&self, pending: Arc<Pending>) -> StoreResult<()> {
        let until = tokio::time::Instant::from_std(pending.start_until);
        let room = tokio::time::timeout_at(until, Arc::clone(&self.room).acquire_owned())
            .await
            .map_err(|_| StoreError::ConnectionTimeout)?
            .map_err(|_| StoreError::TaskFailed)?;
        *pending.room.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(room);
        self.sender.send(pending).map_err(|_| StoreError::TaskFailed)
    }
}

/// Worker loop: runs tasks one at a time until the store is dropped. A task that becomes running
/// only after the time it could start until is cancelled instead; a panicking task is contained
/// here (its caller sees `TaskFailed`).
fn serve(receiver: &Mutex<mpsc::Receiver<Arc<Pending>>>, mut slot: Slot) {
    loop {
        let pending = match receiver.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).recv()
        {
            Ok(pending) => pending,
            Err(_) => return,
        };
        drop(pending.room.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).take());
        // Cancelled while queued: its closure is already gone.
        if !transition(&pending.state, QUEUED, RUNNING) {
            continue;
        }
        // Checked after the state change, so a task that becomes running late never starts,
        // whether or not its caller has noticed the time yet.
        if Instant::now() > pending.start_until {
            pending.state.store(CANCELLED, Ordering::SeqCst);
            discard(pending.take_job());
            continue;
        }
        if let Some(job) = pending.take_job() {
            let _ = catch_unwind(AssertUnwindSafe(|| job(&mut slot)));
        }
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
    pending: Arc<Pending>,
    armed: bool,
}

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        if self.armed && !self.pending.cancel_if_queued() {
            transition(&self.pending.state, RUNNING, CANCELLED);
        }
    }
}

/// Runs `body` on a new thread and waits for its result without blocking the async runtime.
async fn on_thread<R: Send + 'static>(
    body: impl FnOnce() -> StoreResult<R> + Send + 'static,
) -> StoreResult<R> {
    let (reply, receive) = oneshot::channel();
    std::thread::Builder::new().name("rssr-library-open".to_owned()).spawn(move || {
        let _ = reply.send(body());
    })?;
    receive.await.map_err(|_| StoreError::TaskFailed)?
}

/// Creates the library at `path` if it does not exist: built in a temporary file next to it and
/// published only when complete, never over an existing file.
fn create_if_missing(path: &Path) -> StoreResult<()> {
    if path.exists() {
        return Ok(());
    }
    let lock = files::CreationLock::acquire(path)?;
    // Another creator (an import, another process) may have finished while this one waited.
    if path.exists() {
        return Ok(());
    }
    let staging = lock.scratch("creating");
    let mut connection = open_connection(Some(staging.path()), Role::Writer)?;
    let context = TaskContext {
        state: Arc::new(AtomicU8::new(RUNNING)),
        until: Instant::now() + connection::BUSY_TIMEOUT,
    };
    with_context(context, || migrate::migrate(&mut connection, &now()))?;
    connection.close().map_err(|(_, error)| error)?;
    lock.publish(staging)
}

impl LibraryStore {
    /// Opens (creating if needed) the library at `path` and brings its schema up to date.
    pub async fn open(path: impl AsRef<Path>) -> StoreResult<Self> {
        Self::open_with(path, StoreOptions::default()).await
    }

    pub async fn open_with(path: impl AsRef<Path>, options: StoreOptions) -> StoreResult<Self> {
        let path = path.as_ref().to_path_buf();
        on_thread(move || Self::open_file(&path, options)).await
    }

    /// A private in-memory library on a single connection (tests, throwaway use).
    pub async fn open_in_memory() -> StoreResult<Self> {
        let options = StoreOptions { readers: 0, ..StoreOptions::default() };
        on_thread(move || {
            let mut writer = open_connection(None, Role::Writer)?;
            let context = TaskContext {
                state: Arc::new(AtomicU8::new(RUNNING)),
                until: Instant::now() + connection::BUSY_TIMEOUT,
            };
            with_context(context, || migrate::migrate(&mut writer, &now()))?;
            let writer = Queue::start(
                vec![Slot { connection: Some(writer), path: None, role: Role::Writer }],
                options.queue_capacity,
            )?;
            Ok(Self::assemble(writer, None, "memory".to_owned(), options))
        })
        .await
    }

    fn open_file(path: &Path, options: StoreOptions) -> StoreResult<Self> {
        // One interpretation of the path for the probe, creation and every connection; a path
        // SQLite cannot be given as a URI is refused before anything is created.
        let path = &std::path::absolute(path)?;
        let probe = files::probe_uri(path)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        create_if_missing(path)?;
        files::check_library(&probe)?;
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
        let capacity = options.queue_capacity;
        let readers =
            if readers.is_empty() { None } else { Some(Queue::start(readers, capacity)?) };
        let writer = Queue::start(
            vec![Slot {
                connection: Some(writer),
                path: Some(path.to_path_buf()),
                role: Role::Writer,
            }],
            capacity,
        )?;
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
    /// before its commit point (the commit itself can still fail).
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
        let job: Job = Box::new(move |slot: &mut Slot| {
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
        });
        let start_until = Instant::now() + self.inner.options.admission_wait;
        let pending = Arc::new(Pending {
            state: Arc::clone(&state),
            start_until,
            job: Mutex::new(Some(job)),
            room: Mutex::new(None),
        });
        if let Err(error) = queue.submit(Arc::clone(&pending)).await {
            // Never queued: drop its closure here, contained like any other discarded one.
            discard(pending.take_job());
            return Err(error);
        }
        let mut cancel = CancelOnDrop { pending, armed: true };
        let until = tokio::time::Instant::from_std(start_until);
        let result = match tokio::time::timeout_at(until, &mut receive).await {
            Ok(result) => result,
            Err(_) if cancel.pending.cancel_if_queued() => {
                cancel.armed = false;
                return Err(StoreError::ConnectionTimeout);
            }
            Err(_) => receive.await,
        };
        cancel.armed = false;
        match result {
            Ok(result) => result,
            // The worker cancelled it: it was taken after the time it could start until.
            Err(_) if state.load(Ordering::SeqCst) == CANCELLED => {
                Err(StoreError::ConnectionTimeout)
            }
            Err(_) => Err(StoreError::TaskFailed),
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn pending(start_until: Instant) -> Arc<Pending> {
        Arc::new(Pending {
            state: Arc::new(AtomicU8::new(QUEUED)),
            start_until,
            job: Mutex::new(None),
            room: Mutex::new(None),
        })
    }

    /// Cells not yet taken by a worker, cancelled or not, are bounded by the capacity; taking one
    /// makes room.
    #[tokio::test]
    async fn a_queue_holds_at_most_its_capacity_until_a_worker_takes_tasks() {
        let (sender, receiver) = mpsc::channel();
        let queue = Queue { sender, room: Arc::new(Semaphore::new(2)) };
        let soon = || Instant::now() + Duration::from_millis(200);
        for _ in 0..2 {
            let queued = pending(soon());
            queue.submit(Arc::clone(&queued)).await.unwrap();
            assert!(queued.cancel_if_queued(), "cancelled cells keep their room");
        }
        assert!(matches!(queue.submit(pending(soon())).await, Err(StoreError::ConnectionTimeout)));

        // What the worker does when it takes a cell.
        let taken = receiver.recv().unwrap();
        drop(taken.room.lock().unwrap().take());
        queue.submit(pending(soon())).await.unwrap();
        assert!(matches!(queue.submit(pending(soon())).await, Err(StoreError::ConnectionTimeout)));
    }
}
