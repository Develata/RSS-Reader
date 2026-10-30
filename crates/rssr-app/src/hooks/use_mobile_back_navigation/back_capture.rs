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

/// Keep the existing guard alive through capture and navigation, including when
/// the UI task is cancelled before its first poll. Capture owns its timeout and
/// error reporting; neither failure nor timeout is a reason to suppress Back.
pub(super) async fn finish_capture(
    guard: BackCaptureGuard<'_>,
    capture: impl std::future::Future,
    route_unchanged: impl FnOnce() -> bool,
    navigate: impl FnOnce(),
) {
    let _guard = guard;
    let _ = capture.await;
    if route_unchanged() {
        navigate();
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

    #[tokio::test]
    async fn capture_success_and_failure_both_navigate_before_releasing() {
        let gate = BackCaptureGate::new();
        for result in [Ok(()), Err(())] {
            let navigated = std::cell::Cell::new(false);
            finish_capture(
                gate.try_acquire().unwrap(),
                std::future::ready(result),
                || true,
                || {
                    assert!(gate.try_acquire().is_none());
                    navigated.set(true);
                },
            )
            .await;
            assert!(navigated.get());
            assert!(gate.try_acquire().is_some());
        }
    }

    #[tokio::test]
    async fn timed_out_capture_still_navigates_and_releases() {
        let gate = BackCaptureGate::new();
        let navigated = std::cell::Cell::new(false);
        finish_capture(
            gate.try_acquire().unwrap(),
            tokio::time::timeout(std::time::Duration::ZERO, std::future::pending::<()>()),
            || true,
            || navigated.set(true),
        )
        .await;
        assert!(navigated.get());
        assert!(gate.try_acquire().is_some());
    }

    #[tokio::test]
    async fn route_changed_while_waiting_discards_the_old_request_and_releases() {
        let gate = BackCaptureGate::new();
        let unchanged = std::cell::Cell::new(true);
        let (ready, waiting) = tokio::sync::oneshot::channel::<()>();
        let mut request = Box::pin(finish_capture(
            gate.try_acquire().unwrap(),
            waiting,
            || unchanged.get(),
            || panic!("old route must not navigate"),
        ));
        assert!(
            std::future::poll_fn(|cx| std::task::Poll::Ready(request.as_mut().poll(cx)))
                .await
                .is_pending()
        );
        assert!(gate.try_acquire().is_none(), "waiting requests coalesce");
        unchanged.set(false);
        ready.send(()).unwrap();
        request.await;
        assert!(gate.try_acquire().is_some());
    }

    #[tokio::test]
    async fn cancellation_before_or_after_poll_releases_without_navigation() {
        let gate = BackCaptureGate::new();
        for poll in [false, true] {
            let mut request = Box::pin(finish_capture(
                gate.try_acquire().unwrap(),
                std::future::pending::<()>(),
                || true,
                || panic!("cancelled capture must not navigate"),
            ));
            if poll {
                assert!(
                    std::future::poll_fn(|cx| std::task::Poll::Ready(request.as_mut().poll(cx)))
                        .await
                        .is_pending()
                );
            }
            assert!(gate.try_acquire().is_none());
            drop(request);
            assert!(gate.try_acquire().is_some());
        }
    }
}
