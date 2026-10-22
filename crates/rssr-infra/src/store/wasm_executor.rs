//! A private memory connection on the page thread. SQL and repository transactions are the
//! same as native. Handles contain only an id and synchronization state, never JS or SQLite.
//!
//! Each admitted task yields to the browser before starting. Dropping its caller discards a
//! queued closure immediately. Once synchronous SQL starts, the event loop cannot poll another
//! caller; its progress handler enforces the execution deadline. Cancellation is still checked
//! at the commit point. No worker, persistent VFS, Web Lock or browser storage is involved.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::future::{Either, select};
use rusqlite::{Connection, Transaction, TransactionBehavior};
use tokio::sync::{Notify, Semaphore, oneshot};

use super::connection::{self, Role};
pub use super::options::StoreOptions;
use super::{StoreError, StoreResult, codec, migrate};

const QUEUED: u8 = 0;
const RUNNING: u8 = 1;
const COMMITTING: u8 = 2;
const CANCELLED: u8 = 3;
const PROGRESS_OPS: i32 = 1_000;

thread_local! {
    static CONNECTIONS: RefCell<BTreeMap<usize, Connection>> = const { RefCell::new(BTreeMap::new()) };
}
static NEXT_ID: AtomicUsize = AtomicUsize::new(1);

#[derive(Clone)]
pub struct LibraryStore {
    inner: Arc<Inner>,
}

struct Inner {
    id: usize,
    options: StoreOptions,
    room: Arc<Semaphore>,
    closed: AtomicBool,
    active: AtomicUsize,
    finished: Notify,
}

impl Drop for Inner {
    fn drop(&mut self) {
        CONNECTIONS.with(|connections| connections.borrow_mut().remove(&self.id));
    }
}

struct Pending<F> {
    state: AtomicU8,
    task: Mutex<Option<F>>,
    cancelled: Notify,
}

struct CancelOnDrop<F>(Arc<Pending<F>>);

impl<F> Drop for CancelOnDrop<F> {
    fn drop(&mut self) {
        if self
            .0
            .state
            .compare_exchange(QUEUED, CANCELLED, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            // Do not retain an abandoned task's captured data until its timer or permit wakes.
            self.0.task.lock().unwrap().take();
            self.0.cancelled.notify_one();
        } else {
            let _ = self.0.state.compare_exchange(
                RUNNING,
                CANCELLED,
                Ordering::SeqCst,
                Ordering::SeqCst,
            );
        }
    }
}

struct ActiveTask(Arc<Inner>);

impl Drop for ActiveTask {
    fn drop(&mut self) {
        self.0.active.fetch_sub(1, Ordering::SeqCst);
        self.0.finished.notify_waiters();
    }
}

impl LibraryStore {
    pub async fn open_in_memory() -> StoreResult<Self> {
        Self::open_in_memory_with(StoreOptions::default()).await
    }

    pub async fn open_in_memory_with(options: StoreOptions) -> StoreResult<Self> {
        let mut connection = Connection::open_in_memory()?;
        connection::configure(&connection, Role::Writer)?;
        // A private single connection has no other lock owner to wait for.
        connection.busy_timeout(Duration::ZERO)?;
        let until = js_sys::Date::now() + connection::BUSY_TIMEOUT.as_secs_f64() * 1000.0;
        connection.progress_handler(PROGRESS_OPS, Some(move || js_sys::Date::now() >= until))?;
        migrate::migrate(&mut connection, &codec::now())?;
        connection.progress_handler(0, None::<fn() -> bool>)?;
        let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
        CONNECTIONS.with(|connections| connections.borrow_mut().insert(id, connection));
        Ok(Self {
            inner: Arc::new(Inner {
                id,
                room: Arc::new(Semaphore::new(options.queue_capacity.max(1))),
                options,
                closed: AtomicBool::new(false),
                active: AtomicUsize::new(0),
                finished: Notify::new(),
            }),
        })
    }

    pub fn journal_mode(&self) -> &str {
        "memory"
    }

    /// Reject new work, finish already submitted tasks, then release this page's connection.
    pub async fn close(&self) -> StoreResult<()> {
        self.inner.closed.store(true, Ordering::SeqCst);
        loop {
            let finished = self.inner.finished.notified();
            tokio::pin!(finished);
            finished.as_mut().enable();
            if self.inner.active.load(Ordering::SeqCst) == 0 {
                break;
            }
            finished.await;
        }
        CONNECTIONS.with(|connections| -> StoreResult<()> {
            if let Some(connection) = connections.borrow_mut().remove(&self.inner.id) {
                connection.close().map_err(|(_, error)| error)?;
            }
            Ok(())
        })
    }

    pub async fn read<R, F>(&self, task: F) -> StoreResult<R>
    where
        R: Send + 'static,
        F: FnOnce(&Connection) -> StoreResult<R> + Send + 'static,
    {
        self.run(TransactionBehavior::Deferred, self.inner.options.read_deadline, true, move |tx| {
            task(tx)
        })
        .await
    }

    pub async fn write<R, F>(&self, task: F) -> StoreResult<R>
    where
        R: Send + 'static,
        F: FnOnce(&Transaction<'_>) -> StoreResult<R> + Send + 'static,
    {
        self.run(TransactionBehavior::Immediate, self.inner.options.write_deadline, false, task)
            .await
    }

    async fn run<R, F>(
        &self,
        behavior: TransactionBehavior,
        deadline: Duration,
        read_only: bool,
        task: F,
    ) -> StoreResult<R>
    where
        R: Send + 'static,
        F: FnOnce(&Transaction<'_>) -> StoreResult<R> + Send + 'static,
    {
        if self.inner.closed.load(Ordering::SeqCst) {
            return Err(StoreError::Closed);
        }
        let start_until =
            js_sys::Date::now() + self.inner.options.admission_wait.as_secs_f64() * 1000.0;
        let pending = Arc::new(Pending {
            state: AtomicU8::new(QUEUED),
            task: Mutex::new(Some(task)),
            cancelled: Notify::new(),
        });
        let _cancel = CancelOnDrop(Arc::clone(&pending));
        let (reply, receive) = oneshot::channel();
        let inner = Arc::clone(&self.inner);
        inner.active.fetch_add(1, Ordering::SeqCst);
        // The bridge keeps repository futures Send; only this local task owns browser timers.
        wasm_bindgen_futures::spawn_local(async move {
            let _active = ActiveTask(Arc::clone(&inner));
            if pending.state.load(Ordering::SeqCst) == CANCELLED {
                return;
            }
            let wait = gloo_timers::future::sleep(inner.options.admission_wait);
            let acquire = Arc::clone(&inner.room).acquire_owned();
            futures_util::pin_mut!(wait, acquire);
            let admitted = select(acquire, wait);
            futures_util::pin_mut!(admitted);
            let permit = match select(admitted, Box::pin(pending.cancelled.notified())).await {
                Either::Right(_) => return,
                Either::Left((admitted, _)) => match admitted {
                    Either::Left((Ok(permit), _)) => permit,
                    _ => {
                        pending.state.store(CANCELLED, Ordering::SeqCst);
                        pending.task.lock().unwrap().take();
                        let _ = reply.send(Err(StoreError::ConnectionTimeout));
                        return;
                    }
                },
            };
            if matches!(
                select(
                    gloo_timers::future::sleep(Duration::ZERO),
                    Box::pin(pending.cancelled.notified())
                )
                .await,
                Either::Right(_)
            ) {
                return;
            }
            drop(permit);
            if js_sys::Date::now() >= start_until
                || pending
                    .state
                    .compare_exchange(QUEUED, RUNNING, Ordering::SeqCst, Ordering::SeqCst)
                    .is_err()
            {
                pending.state.store(CANCELLED, Ordering::SeqCst);
                pending.task.lock().unwrap().take();
                let _ = reply.send(Err(StoreError::ConnectionTimeout));
                return;
            }
            let task = pending.task.lock().unwrap().take().expect("running task");
            let until = js_sys::Date::now() + deadline.as_secs_f64() * 1000.0;
            let result = CONNECTIONS.with(|connections| -> StoreResult<R> {
                let mut connections =
                    connections.try_borrow_mut().map_err(|_| StoreError::TaskFailed)?;
                let connection = connections.get_mut(&inner.id).ok_or(StoreError::Closed)?;
                let state = Arc::clone(&pending);
                connection.progress_handler(
                    PROGRESS_OPS,
                    Some(move || {
                        state.state.load(Ordering::SeqCst) == CANCELLED
                            || js_sys::Date::now() >= until
                    }),
                )?;
                let result = (|| {
                    connection.pragma_update(None, "query_only", read_only)?;
                    let tx = connection.transaction_with_behavior(behavior)?;
                    let value = task(&tx)?;
                    if pending
                        .state
                        .compare_exchange(RUNNING, COMMITTING, Ordering::SeqCst, Ordering::SeqCst)
                        .is_err()
                    {
                        return Err(StoreError::Interrupted);
                    }
                    tx.commit()?;
                    Ok(value)
                })();
                // Cleanup must not inherit an expired handler. A damaged memory connection
                // cannot be reopened without silently losing the library: discard it instead.
                let reset = (|| -> StoreResult<()> {
                    connection.progress_handler(0, None::<fn() -> bool>)?;
                    if !connection.is_autocommit() {
                        connection.execute_batch("ROLLBACK")?;
                    }
                    connection.pragma_update(None, "query_only", false)?;
                    if !connection.is_autocommit() {
                        return Err(StoreError::TaskFailed);
                    }
                    Ok(())
                })();
                if reset.is_err() {
                    connections.remove(&inner.id);
                }
                result.and_then(|value| reset.map(|()| value))
            });
            let _ = reply.send(result);
        });
        receive.await.map_err(|_| StoreError::TaskFailed)?
    }
}
