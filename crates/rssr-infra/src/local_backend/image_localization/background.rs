//! Transient scheduling only: no body copies, persistent failure flags or second
//! article store. Cursors improve fairness within a running host; eviction or
//! restart may retry an earlier article, never mark a failed image as successful.
use std::collections::{HashSet, VecDeque};
use std::sync::{Arc, Mutex, PoisonError};

use rssr_application::RefreshLocalizationCandidate;
use sha2::{Digest, Sha256};
use tokio::sync::Semaphore;

const MAX_PENDING_FEEDS: usize = 64;
const MAX_PENDING_BYTES: usize = 8 * 1024 * 1024;
const MAX_JOB_BYTES: usize = 256 * 1024;
const MAX_JOB_CANDIDATES: usize = 256;
const MAX_CURSORS: usize = 256;

pub(super) struct BackgroundQueue {
    pub slots: Semaphore,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    pending: HashSet<i64>,
    payload_bytes: usize,
    cursors: VecDeque<Cursor>,
}

struct Cursor {
    feed_id: i64,
    generation: i64,
    // Fixed-size identity; long source IDs cannot enlarge the cursor cache.
    after_key: [u8; 32],
}

/// Admission is released even when the waiting/running task is aborted.
pub(super) struct PendingFeed {
    queue: Arc<BackgroundQueue>,
    feed_id: i64,
    payload_bytes: usize,
}

impl BackgroundQueue {
    pub fn new(concurrency: usize) -> Self {
        Self { slots: Semaphore::new(concurrency), state: Mutex::new(State::default()) }
    }

    pub fn enqueue(
        self: &Arc<Self>,
        feed_id: i64,
        mut entries: Vec<RefreshLocalizationCandidate>,
    ) -> Option<(PendingFeed, Vec<RefreshLocalizationCandidate>)> {
        let generation = entries.first()?.generation;
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if state.pending.contains(&feed_id) || state.pending.len() >= MAX_PENDING_FEEDS {
            return None;
        }
        let after_key = state
            .cursors
            .iter()
            .find(|cursor| cursor.feed_id == feed_id && cursor.generation == generation)
            .map(|cursor| cursor.after_key);
        drop(state);

        if let Some(index) = after_key.and_then(|after| {
            entries.iter().position(|entry| key_digest(&entry.dedup_key) == after)
        }) {
            let start = (index + 1) % entries.len();
            entries.rotate_left(start);
        }
        // Allocate a fresh bounded vector: truncating `entries` would retain the
        // original feed's entire vector capacity while waiting for a slot.
        let mut job = Vec::with_capacity(MAX_JOB_CANDIDATES.min(entries.len()));
        let mut payload_bytes =
            job.capacity() * std::mem::size_of::<RefreshLocalizationCandidate>();
        for entry in entries {
            let bytes = entry.dedup_key.capacity()
                + entry.source_content_hash.capacity()
                + entry.url.as_ref().map_or(0, |url| url.as_str().len());
            if bytes + std::mem::size_of::<RefreshLocalizationCandidate>() > MAX_JOB_BYTES {
                continue;
            }
            if payload_bytes + bytes > MAX_JOB_BYTES && job.is_empty() {
                continue;
            }
            if job.len() == MAX_JOB_CANDIDATES || payload_bytes + bytes > MAX_JOB_BYTES {
                break;
            }
            payload_bytes += bytes;
            job.push(entry);
        }
        if job.is_empty() {
            return None;
        }
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if state.pending.contains(&feed_id)
            || state.pending.len() >= MAX_PENDING_FEEDS
            || state.payload_bytes + payload_bytes > MAX_PENDING_BYTES
        {
            return None;
        }
        state.pending.insert(feed_id);
        state.payload_bytes += payload_bytes;
        Some((PendingFeed { queue: Arc::clone(self), feed_id, payload_bytes }, job))
    }
}

impl PendingFeed {
    /// Advance before awaiting: a timed-out request must not pin the next round
    /// to the same first article. No-image, stale and derived bodies advance too.
    pub fn examined(&self, entry: &RefreshLocalizationCandidate) {
        let cursor = Cursor {
            feed_id: self.feed_id,
            generation: entry.generation,
            after_key: key_digest(&entry.dedup_key),
        };
        let mut state = self.queue.state.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(index) = state.cursors.iter().position(|old| old.feed_id == self.feed_id) {
            state.cursors.remove(index);
        }
        if state.cursors.len() == MAX_CURSORS {
            state.cursors.pop_front();
        }
        state.cursors.push_back(cursor);
    }
}

impl Drop for PendingFeed {
    fn drop(&mut self) {
        let mut state = self.queue.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.pending.remove(&self.feed_id);
        state.payload_bytes -= self.payload_bytes;
    }
}

fn key_digest(key: &str) -> [u8; 32] {
    Sha256::digest(key.as_bytes()).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rssr_application::ParsedEntryData;

    fn entries(
        count: usize,
        generation: i64,
        key_bytes: usize,
    ) -> Vec<RefreshLocalizationCandidate> {
        (0..count)
            .map(|i| {
                RefreshLocalizationCandidate::from_entry(
                    &ParsedEntryData {
                        external_id: String::new(),
                        dedup_key: format!("{i}-{}", "k".repeat(key_bytes)),
                        url: None,
                        title: String::new(),
                        author: None,
                        summary: None,
                        content_html: Some("<p>source</p>".into()),
                        content_text: None,
                        published_at: None,
                        updated_at_source: None,
                    },
                    generation,
                )
                .unwrap()
            })
            .collect()
    }

    #[test]
    fn admission_bounds_jobs_and_releases_on_drop() {
        let queue = Arc::new(BackgroundQueue::new(2));
        let mut pending = Vec::new();
        for feed in 0..MAX_PENDING_FEEDS as i64 {
            let (guard, job) = queue.enqueue(feed, entries(1, 0, 0)).unwrap();
            assert_eq!(job.len(), 1);
            pending.push(guard);
        }
        assert!(queue.enqueue(999, entries(1, 0, 0)).is_none());
        assert!(queue.enqueue(0, entries(1, 0, 0)).is_none());
        drop(pending);
        assert_eq!(queue.state.lock().unwrap().payload_bytes, 0);
        assert!(queue.enqueue(999, entries(1, 0, 0)).is_some());
    }

    #[test]
    fn large_feeds_and_long_keys_have_bounded_retained_payload() {
        let queue = Arc::new(BackgroundQueue::new(2));
        let (guard, job) = queue.enqueue(1, entries(10_000, 0, 1000)).unwrap();
        assert!(job.len() <= MAX_JOB_CANDIDATES);
        assert!(job.capacity() <= MAX_JOB_CANDIDATES);
        assert!(guard.payload_bytes <= MAX_JOB_BYTES);
        let mut pending = vec![guard];
        for feed in 2..100 {
            if let Some((guard, _)) = queue.enqueue(feed, entries(256, 0, 1000)) {
                pending.push(guard);
            }
        }
        let state = queue.state.lock().unwrap();
        assert!(state.payload_bytes <= MAX_PENDING_BYTES);
        assert!(state.pending.len() < MAX_PENDING_FEEDS, "byte admission was exercised");
        drop(state);
        drop(pending);
        assert_eq!(queue.state.lock().unwrap().payload_bytes, 0);
    }

    #[test]
    fn progress_survives_failure_and_ignores_old_generations() {
        let queue = Arc::new(BackgroundQueue::new(2));
        let (guard, first) = queue.enqueue(1, entries(8, 0, 0)).unwrap();
        guard.examined(&first[4]);
        drop(guard);
        let (guard, next) = queue.enqueue(1, entries(8, 0, 0)).unwrap();
        assert_eq!(next[0].dedup_key, "5-");
        guard.examined(&next[2]);
        drop(guard);
        let (guard, wrapped) = queue.enqueue(1, entries(8, 0, 0)).unwrap();
        assert_eq!(wrapped[0].dedup_key, "0-");
        guard.examined(&wrapped[4]);
        drop(guard);
        let (_, reset) = queue.enqueue(1, entries(8, 1, 0)).unwrap();
        assert_eq!(reset[0].dedup_key, "0-");
    }

    #[test]
    fn cursor_cache_is_bounded() {
        let queue = Arc::new(BackgroundQueue::new(2));
        for feed in 0..MAX_CURSORS as i64 + 20 {
            let (guard, entries) = queue.enqueue(feed, entries(1, 0, 0)).unwrap();
            guard.examined(&entries[0]);
        }
        assert_eq!(queue.state.lock().unwrap().cursors.len(), MAX_CURSORS);
    }
}
