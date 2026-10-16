//! Background jobs (v0.2 phase C, C3; decision Q2): refreshing every subscription, refreshing
//! one, adding one (with its first refresh) and the WebDAV upload and download run here, not
//! inside a request.
//!
//! - Admission comes before any side effect: at most [`MAX_RUNNING`] jobs run and [`MAX_QUEUED`]
//!   wait; another is refused at once (`unavailable`) and its work never starts.
//! - A job holds its running slot until it ends, whether or not anyone polls it, and runs on
//!   when the client that started it goes away.
//! - A finished job's status is serialized once. It is kept for [`RETENTION`] after the job
//!   finished; at most [`MAX_RESULTS`] statuses, [`MAX_RETAINED_BYTES`] in all, are kept, and
//!   the oldest finished go first. A job whose status is gone (expired, evicted, or from before
//!   a restart) is `status_unavailable`: the retention is a bound, not a promise.
//! - Ids are 128 random bits.
//! - A poll waits at most [`protocol::MAX_JOB_WAIT`] for the job to finish.

use std::{
    collections::{HashMap, VecDeque},
    future::Future,
    sync::{Arc, Mutex, MutexGuard},
    time::Duration,
};

use axum::body::Bytes;
use rand_core::{OsRng, RngCore};
use rssr_api::{
    BackendError, ErrorCode,
    protocol::{self, JobId, JobResult, JobStatus},
};
use rssr_application::{MAX_MESSAGE_BYTES, truncate_marked};
use tokio::{
    sync::{Semaphore, watch},
    time::Instant,
};
use tracing::{error, warn};

/// Jobs running at once.
pub(crate) const MAX_RUNNING: usize = 2;
/// Jobs waiting to run.
pub(crate) const MAX_QUEUED: usize = 8;
/// How long a finished job's status is kept at most.
const RETENTION: Duration = Duration::from_secs(10 * 60);
/// Finished jobs' statuses kept at most.
const MAX_RESULTS: usize = 32;
/// Bytes of finished jobs' statuses kept at most (a single one is at most about 256 KiB:
/// `rssr_application::MAX_RESULT_BYTES`).
const MAX_RETAINED_BYTES: usize = 4 * 1024 * 1024;

/// The status of a job that failed without a status of its own.
const INTERNAL_FAILURE: &str = r#"{"state":"failed","error":{"code":"internal","message":"服务器内部错误","request_id":null}}"#;

/// What pollers of a job see; the final status is serialized once.
#[derive(Clone)]
enum Phase {
    Queued,
    Running,
    Done(Bytes),
}

/// The server's background jobs. Cheap to clone.
#[derive(Clone)]
pub(crate) struct Jobs {
    shared: Arc<Shared>,
}

struct Shared {
    table: Mutex<Table>,
    running: Arc<Semaphore>,
}

#[derive(Default)]
struct Table {
    /// Queued and running jobs.
    active: HashMap<JobId, watch::Receiver<Phase>>,
    /// Finished jobs' statuses and when they finished.
    finished: HashMap<JobId, (Bytes, Instant)>,
    /// Finished jobs, oldest first.
    order: VecDeque<JobId>,
    /// Bytes of the statuses in `finished`.
    retained: usize,
}

impl Table {
    /// Drops the statuses kept longer than [`RETENTION`].
    ///
    /// Invariant: `order` is in finishing order, so the expired ones are at its front.
    fn expire(&mut self, now: Instant) {
        while let Some(oldest) = self.order.front() {
            let finished_at = self.finished[oldest].1;
            if now.duration_since(finished_at) < RETENTION {
                break;
            }
            self.evict_oldest();
        }
    }

    fn evict_oldest(&mut self) {
        if let Some(id) = self.order.pop_front()
            && let Some((status, _)) = self.finished.remove(&id)
        {
            self.retained -= status.len();
        }
    }

    /// Keeps `status` as the job's final one, then evicts by age, count and size.
    fn finish(&mut self, id: JobId, status: Bytes, now: Instant) {
        self.active.remove(&id);
        self.retained += status.len();
        self.finished.insert(id.clone(), (status, now));
        self.order.push_back(id);
        self.expire(now);
        while self.order.len() > MAX_RESULTS || self.retained > MAX_RETAINED_BYTES {
            self.evict_oldest();
        }
    }

    fn knows(&self, id: &JobId) -> bool {
        self.active.contains_key(id) || self.finished.contains_key(id)
    }
}

impl Jobs {
    pub(crate) fn new() -> Self {
        Self {
            shared: Arc::new(Shared {
                table: Mutex::default(),
                running: Arc::new(Semaphore::new(MAX_RUNNING)),
            }),
        }
    }

    fn table(&self) -> MutexGuard<'_, Table> {
        lock(&self.shared.table)
    }

    /// Admits `job` and runs it in the background, or refuses it at once when [`MAX_RUNNING`]
    /// run and [`MAX_QUEUED`] wait: `job` is then dropped without being polled, so nothing of it
    /// happened. `request_id` (the request that started it) goes on a failure and in the log.
    pub(crate) fn submit<F>(&self, request_id: &str, job: F) -> Result<JobId, BackendError>
    where
        F: Future<Output = Result<JobResult, BackendError>> + Send + 'static,
    {
        let (sender, receiver) = watch::channel(Phase::Queued);
        let id = {
            let mut table = self.table();
            table.expire(Instant::now());
            if table.active.len() >= MAX_RUNNING + MAX_QUEUED {
                return Err(BackendError::new(ErrorCode::Unavailable, "后台任务已满，请稍后再试"));
            }
            let id = loop {
                let id = new_id();
                if !table.knows(&id) {
                    break id;
                }
            };
            table.active.insert(id.clone(), receiver);
            id
        };
        let shared = Arc::clone(&self.shared);
        let job_id = id.clone();
        let request_id = request_id.to_owned();
        tokio::spawn(async move {
            let status = match Arc::clone(&shared.running).acquire_owned().await {
                Ok(slot) => {
                    sender.send_replace(Phase::Running);
                    let status = run(job, &job_id, &request_id).await;
                    drop(slot);
                    status
                }
                // The semaphore is never closed; were it, the job would fail rather than wait.
                Err(_) => internal_failure(&request_id),
            };
            let status = serialize(&status);
            lock(&shared.table).finish(job_id, status.clone(), Instant::now());
            sender.send_replace(Phase::Done(status));
        });
        Ok(id)
    }

    /// Test only: admits jobs that never finish until no more are admitted.
    #[cfg(test)]
    pub(crate) fn fill(&self) {
        while self.submit("fill", std::future::pending()).is_ok() {}
    }

    /// The job's status as JSON, after waiting up to `wait` (at most
    /// [`protocol::MAX_JOB_WAIT`]) for it to finish; `None` when its status is gone.
    pub(crate) async fn status(&self, id: &JobId, wait: Duration) -> Option<Bytes> {
        let mut receiver = {
            let mut table = self.table();
            table.expire(Instant::now());
            if let Some((status, _)) = table.finished.get(id) {
                return Some(status.clone());
            }
            table.active.get(id)?.clone()
        };
        let finished = |phase: &Phase| matches!(phase, Phase::Done(_));
        let wait = wait.min(protocol::MAX_JOB_WAIT);
        // Times out, or the job finished; the sender sends `Done` before it goes away.
        let _ = tokio::time::timeout(wait, receiver.wait_for(finished)).await;
        let phase = receiver.borrow().clone();
        Some(match phase {
            Phase::Queued => serialize(&JobStatus::Queued),
            Phase::Running => serialize(&JobStatus::Running),
            Phase::Done(status) => status,
        })
    }
}

/// Runs `job` to its status. In its own task, so a panic fails the job instead of leaving it
/// running forever.
async fn run<F>(job: F, job_id: &JobId, request_id: &str) -> JobStatus
where
    F: Future<Output = Result<JobResult, BackendError>> + Send + 'static,
{
    match tokio::spawn(job).await {
        Ok(Ok(result)) => JobStatus::Succeeded { result },
        Ok(Err(error)) => {
            // The message may quote a subscription's address with its credentials: not logged.
            warn!(job_id = %job_id.0, request_id, code = error.code.as_str(), "后台任务失败");
            let message = truncate_marked(error.message, MAX_MESSAGE_BYTES);
            JobStatus::Failed {
                error: BackendError { message, ..error }.with_request_id(request_id),
            }
        }
        Err(join) => {
            error!(job_id = %job_id.0, request_id, %join, "后台任务异常结束");
            internal_failure(request_id)
        }
    }
}

fn internal_failure(request_id: &str) -> JobStatus {
    let error = BackendError::new(ErrorCode::Internal, "服务器内部错误");
    JobStatus::Failed { error: error.with_request_id(request_id) }
}

fn lock(table: &Mutex<Table>) -> MutexGuard<'_, Table> {
    // The table is consistent between statements; a panic elsewhere does not poison its use.
    table.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// 128 random bits as 32 lowercase hex digits.
fn new_id() -> JobId {
    let mut bytes = [0_u8; 16];
    OsRng.fill_bytes(&mut bytes);
    JobId(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn serialize(status: &JobStatus) -> Bytes {
    match serde_json::to_vec(status) {
        Ok(status) => Bytes::from(status),
        Err(error) => {
            error!(%error, "无法编码任务状态");
            Bytes::from_static(INTERNAL_FAILURE.as_bytes())
        }
    }
}
#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use rssr_api::{RefreshAllExecutionOutcome, RefreshFailureItem};
    use rssr_application::{ErrorKind, RemoteConfigPushOutcome};
    use serde_json::Value;
    use tokio::sync::oneshot;

    use super::*;

    fn pushed(count: u64) -> JobResult {
        JobResult::RemoteConfigPush(RemoteConfigPushOutcome { exported_feed_count: count })
    }

    type Finish = oneshot::Sender<Result<JobResult, BackendError>>;

    /// A job that says when it starts and finishes when told.
    struct Controlled {
        started: Arc<AtomicBool>,
        finish: Option<Finish>,
    }

    impl Controlled {
        fn submit(jobs: &Jobs) -> (Result<JobId, BackendError>, Self) {
            let started = Arc::new(AtomicBool::new(false));
            let (finish, finished) = oneshot::channel();
            let flag = Arc::clone(&started);
            let id = jobs.submit("r", async move {
                flag.store(true, Ordering::SeqCst);
                finished.await.unwrap_or_else(|_| Err(BackendError::new(ErrorCode::Internal, "x")))
            });
            (id, Self { started, finish: Some(finish) })
        }

        fn started(&self) -> bool {
            self.started.load(Ordering::SeqCst)
        }

        fn finish(&mut self, result: Result<JobResult, BackendError>) {
            let _ = self.finish.take().unwrap().send(result);
        }
    }

    /// Lets the spawned tasks run up to their next wait.
    async fn settle() {
        for _ in 0..20 {
            tokio::task::yield_now().await;
        }
    }

    async fn state(jobs: &Jobs, id: &JobId) -> Option<Value> {
        let status = jobs.status(id, Duration::ZERO).await?;
        Some(serde_json::from_slice(&status).unwrap())
    }

    /// Two run, eight wait, the eleventh is refused and never starts; a finishing job lets the
    /// next one run, whether or not anyone polls.
    #[tokio::test(start_paused = true)]
    async fn admission_comes_first_and_two_run_at_once() {
        let jobs = Jobs::new();
        let mut admitted: Vec<_> = (0..MAX_RUNNING + MAX_QUEUED)
            .map(|_| {
                let (id, job) = Controlled::submit(&jobs);
                (id.unwrap(), job)
            })
            .collect();
        let (refused, job) = Controlled::submit(&jobs);
        assert_eq!(refused.unwrap_err().code, ErrorCode::Unavailable);
        settle().await;
        assert!(!job.started(), "a refused job never starts");
        let started = admitted.iter().filter(|(_, job)| job.started()).count();
        assert_eq!(started, MAX_RUNNING);
        assert_eq!(state(&jobs, &admitted[0].0).await.unwrap()["state"], "running");
        assert_eq!(state(&jobs, &admitted[2].0).await.unwrap()["state"], "queued");

        admitted[0].1.finish(Ok(pushed(1)));
        settle().await;
        assert!(admitted[2].1.started(), "the first queued job takes the slot");
        assert!(!admitted[3].1.started());
        let done = state(&jobs, &admitted[0].0).await.unwrap();
        assert_eq!(done["state"], "succeeded");
        assert_eq!(done["result"]["outcome"]["exported_feed_count"], 1);
        // Room in the queue again.
        assert!(Controlled::submit(&jobs).0.is_ok());
    }

    /// A failure keeps its code, gets the starting request's id and a bounded message; a panic
    /// fails the job and frees its slot.
    #[tokio::test(start_paused = true)]
    async fn failures_and_panics_end_the_job() {
        let jobs = Jobs::new();
        let long = BackendError::new(ErrorCode::Network, "x".repeat(10 * MAX_MESSAGE_BYTES));
        let failed = jobs.submit("req-1", async move { Err(long) }).unwrap();
        let panicked = jobs.submit("req-2", async { panic!("job panicked") }).unwrap();
        settle().await;
        let status = state(&jobs, &failed).await.unwrap();
        assert_eq!(status["state"], "failed");
        assert_eq!(status["error"]["code"], "network");
        assert_eq!(status["error"]["request_id"], "req-1");
        assert!(status["error"]["message"].as_str().unwrap().len() <= MAX_MESSAGE_BYTES);
        let status = state(&jobs, &panicked).await.unwrap();
        assert_eq!(status["state"], "failed");
        assert_eq!(status["error"]["code"], "internal");
        let (_, next) = Controlled::submit(&jobs);
        settle().await;
        assert!(next.started(), "the panicked job's slot was freed");
    }

    /// A finished status is kept ten minutes at most.
    #[tokio::test(start_paused = true)]
    async fn finished_statuses_expire() {
        let jobs = Jobs::new();
        let id = jobs.submit("r", async { Ok(pushed(0)) }).unwrap();
        settle().await;
        tokio::time::advance(RETENTION - Duration::from_secs(1)).await;
        assert!(state(&jobs, &id).await.is_some());
        tokio::time::advance(Duration::from_secs(1)).await;
        assert!(state(&jobs, &id).await.is_none());
    }

    /// The oldest finished go first: past 32 statuses, and past 4 MiB in all.
    #[tokio::test(start_paused = true)]
    async fn the_oldest_finished_are_evicted() {
        let jobs = Jobs::new();
        let mut ids = Vec::new();
        for count in 0..=MAX_RESULTS as u64 {
            ids.push(jobs.submit("r", async move { Ok(pushed(count)) }).unwrap());
            settle().await;
        }
        assert!(state(&jobs, &ids[0]).await.is_none(), "the 33rd evicts the first");
        assert!(state(&jobs, &ids[1]).await.is_some());

        let jobs = Jobs::new();
        let large = || {
            let failures = (0..200)
                .map(|feed_id| RefreshFailureItem {
                    feed_id,
                    url: format!("https://example.com/{}", "u".repeat(2048)),
                    name: Some("n".repeat(256)),
                    kind: ErrorKind::Network,
                    message: "m".repeat(1024),
                })
                .collect();
            JobResult::RefreshAll(RefreshAllExecutionOutcome::bounded(0, 200, 200, failures))
        };
        let mut ids = Vec::new();
        let mut retained = 0;
        while retained <= MAX_RETAINED_BYTES {
            ids.push(jobs.submit("r", async move { Ok(large()) }).unwrap());
            settle().await;
            retained += jobs.status(ids.last().unwrap(), Duration::ZERO).await.unwrap().len();
        }
        assert!(ids.len() < MAX_RESULTS, "the size bound comes first: {}", ids.len());
        assert!(state(&jobs, &ids[0]).await.is_none());
        assert!(state(&jobs, &ids[1]).await.is_some());
        assert!(state(&jobs, ids.last().unwrap()).await.is_some());
    }

    /// A poll returns as soon as the job finishes, and waits ten seconds at most.
    #[tokio::test(start_paused = true)]
    async fn a_poll_waits_for_the_end_up_to_ten_seconds() {
        let jobs = Jobs::new();
        let (id, mut job) = Controlled::submit(&jobs);
        let id = id.unwrap();
        settle().await;
        let started = Instant::now();
        let status = jobs.status(&id, Duration::from_secs(60)).await.unwrap();
        assert_eq!(started.elapsed(), protocol::MAX_JOB_WAIT);
        assert!(String::from_utf8_lossy(&status).contains("running"));

        let poll = {
            let jobs = jobs.clone();
            let id = id.clone();
            tokio::spawn(async move { jobs.status(&id, Duration::from_secs(10)).await })
        };
        settle().await;
        let started = Instant::now();
        job.finish(Ok(pushed(2)));
        let status = poll.await.unwrap().unwrap();
        assert!(started.elapsed() < Duration::from_secs(1));
        assert!(String::from_utf8_lossy(&status).contains("succeeded"));
    }

    /// Ids are 128 random bits in hex; another table (a restarted server) knows none of them.
    #[tokio::test(start_paused = true)]
    async fn ids_are_random_and_a_restart_forgets_them() {
        let jobs = Jobs::new();
        let ids: Vec<_> =
            (0..5).map(|_| jobs.submit("r", async { Ok(pushed(0)) }).unwrap()).collect();
        for id in &ids {
            assert_eq!(JobId::parse(&id.0).as_ref(), Some(id));
        }
        let unique: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(unique.len(), ids.len());
        settle().await;
        assert!(state(&Jobs::new(), &ids[0]).await.is_none());
    }
}
