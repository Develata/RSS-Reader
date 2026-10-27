use std::time::Duration;

#[derive(Debug, Clone)]
pub struct StoreOptions {
    /// Reader workers for a native file library; memory libraries use one connection.
    pub readers: usize,
    /// Waiting for room and waiting to start share this budget.
    pub admission_wait: Duration,
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
