//! A return request can start in the WebView event thread and finish in the
//! Dioxus task thread. The gate must be shared across both, not thread-local.
use std::sync::atomic::{AtomicBool, Ordering};

pub(super) struct BackCaptureGate(AtomicBool);

impl BackCaptureGate {
    pub(super) const fn new() -> Self {
        Self(AtomicBool::new(false))
    }

    pub(super) fn try_acquire(&self) -> Option<BackCaptureGuard<'_>> {
        self.0
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .ok()
            .map(|_| BackCaptureGuard(self))
    }
}

pub(super) struct BackCaptureGuard<'a>(&'a BackCaptureGate);

impl Drop for BackCaptureGuard<'_> {
    fn drop(&mut self) {
        self.0.0.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completion_on_another_thread_allows_the_next_return() {
        let gate = BackCaptureGate::new();
        for _ in 0..5 {
            let guard = gate.try_acquire().expect("previous request released the gate");
            assert!(gate.try_acquire().is_none());
            std::thread::scope(|scope| {
                scope.spawn(move || drop(guard)).join().unwrap();
            });
        }
        assert!(gate.try_acquire().is_some());
    }

    #[test]
    fn cancelled_unpolled_request_releases_the_gate() {
        let gate = BackCaptureGate::new();
        let guard = gate.try_acquire().unwrap();
        let request = async move {
            let _guard = guard;
            std::future::pending::<()>().await;
        };
        assert!(gate.try_acquire().is_none());
        drop(request);
        assert!(gate.try_acquire().is_some());
    }
}
