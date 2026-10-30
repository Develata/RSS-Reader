//! Only the Android transport's waiter ownership; settings and preview identity stay in session.
use std::sync::{Mutex, PoisonError};

use tokio::sync::oneshot;

#[cfg(target_os = "android")]
mod bridge;
#[cfg(target_os = "android")]
pub(super) use bridge::pick_css_bytes;

type ImportResult = Result<Option<Vec<u8>>, String>;

struct ImportState {
    last_id: i64,
    pending: Option<(i64, oneshot::Sender<ImportResult>)>,
}

struct ImportRequests(Mutex<ImportState>);

impl ImportRequests {
    const fn new() -> Self {
        Self(Mutex::new(ImportState { last_id: 0, pending: None }))
    }

    fn begin(&self, cancel: fn(i64)) -> Result<ImportWait<'_>, String> {
        let mut state = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if state.pending.is_some() {
            return Err("已有主题文件正在载入，请先完成或取消系统选择器。".into());
        }
        let id = state.last_id.checked_add(1).ok_or("Android 主题导入请求编号已耗尽。")?;
        let (sender, receiver) = oneshot::channel();
        state.last_id = id;
        state.pending = Some((id, sender));
        Ok(ImportWait { id, receiver, requests: self, cancel })
    }

    fn take(&self, id: i64) -> Option<oneshot::Sender<ImportResult>> {
        let mut state = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if state.pending.as_ref().is_some_and(|(current, _)| *current == id) {
            state.pending.take().map(|(_, sender)| sender)
        } else {
            None
        }
    }

    fn complete(&self, id: i64, result: ImportResult) {
        // Wake the Dioxus task after releasing the lock; an old callback cannot take a new slot.
        if let Some(sender) = self.take(id) {
            let _ = sender.send(result);
        }
    }
}

struct ImportWait<'a> {
    id: i64,
    receiver: oneshot::Receiver<ImportResult>,
    requests: &'a ImportRequests,
    cancel: fn(i64),
}

impl ImportWait<'_> {
    async fn receive(&mut self) -> ImportResult {
        (&mut self.receiver).await.map_err(|_| "Android 系统选择器未返回导入结果。")?
    }
}

impl Drop for ImportWait<'_> {
    fn drop(&mut self) {
        if self.requests.take(self.id).is_some() {
            // Page disposal and launch failure release the Rust slot even without a callback.
            // The host keeps its physical picker ownership until that picker actually returns.
            (self.cancel)(self.id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicI64, Ordering};

    fn no_cancel(_: i64) {}

    #[tokio::test]
    async fn busy_request_does_not_replace_waiter_and_wrong_callback_does_not_finish_it() {
        let requests = ImportRequests::new();
        let mut wait = requests.begin(no_cancel).unwrap();
        assert!(requests.begin(no_cancel).is_err());
        requests.complete(wait.id + 1, Ok(None));
        assert!(matches!(wait.receiver.try_recv(), Err(oneshot::error::TryRecvError::Empty)));
        requests.complete(wait.id, Ok(Some(b"/* css */".to_vec())));
        assert_eq!(wait.receive().await.unwrap().unwrap(), b"/* css */");
    }

    #[tokio::test]
    async fn dropped_waiter_releases_slot_and_late_callback_cannot_take_replacement() {
        static CANCELLED: AtomicI64 = AtomicI64::new(0);
        let requests = ImportRequests::new();
        let old = requests.begin(|id| CANCELLED.store(id, Ordering::SeqCst)).unwrap();
        let old_id = old.id;
        drop(old); // Also covers a launch that failed before the waiter was polled.
        assert_eq!(CANCELLED.load(Ordering::SeqCst), old_id);
        let mut current = requests.begin(no_cancel).unwrap();
        assert!(current.id > old_id);
        requests.complete(old_id, Err("obsolete failure".into()));
        assert!(matches!(current.receiver.try_recv(), Err(oneshot::error::TryRecvError::Empty)));
        requests.complete(current.id, Ok(None));
        assert_eq!(current.receive().await.unwrap(), None);
    }

    #[tokio::test]
    async fn all_terminal_results_release_slot_before_waiter_is_dropped() {
        for result in [Ok(Some(vec![])), Ok(None), Err("read failed".into())] {
            let requests = ImportRequests::new();
            let mut old =
                requests.begin(|_| panic!("completed request must not cancel host")).unwrap();
            requests.complete(old.id, result.clone());
            let current = requests.begin(no_cancel).unwrap();
            assert_eq!(old.receive().await, result);
            drop(old);
            assert!(requests.begin(no_cancel).is_err(), "old drop must leave new owner intact");
            drop(current);
            assert!(requests.begin(no_cancel).is_ok());
        }
    }

    #[test]
    fn ids_never_wrap_and_reuse_an_old_callback_identity() {
        let requests = ImportRequests::new();
        requests.0.lock().unwrap().last_id = i64::MAX - 1;
        let last = requests.begin(no_cancel).unwrap();
        assert_eq!(last.id, i64::MAX);
        drop(last);
        assert!(requests.begin(no_cancel).is_err());
        assert!(requests.0.lock().unwrap().pending.is_none());
    }
}
