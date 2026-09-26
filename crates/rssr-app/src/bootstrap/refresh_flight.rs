//! Coalesce overlapping all-subscription requests, including the host's auto-refresh.
//! Only an in-flight result is retained; no feed payloads or completed-batch cache.
use std::{future::Future, sync::Mutex};

use tokio::sync::watch;

use super::RefreshAllExecutionOutcome;

type SharedResult = Result<RefreshAllExecutionOutcome, String>;
type Receiver = watch::Receiver<Option<SharedResult>>;

#[derive(Default)]
pub(super) struct RefreshFlight {
    current: Mutex<Option<Receiver>>,
}

impl RefreshFlight {
    pub(super) async fn run(
        &self,
        refresh: impl Future<Output = anyhow::Result<RefreshAllExecutionOutcome>>,
    ) -> anyhow::Result<RefreshAllExecutionOutcome> {
        let (mut receiver, sender) = {
            let mut slot = self.current.lock().unwrap_or_else(|error| error.into_inner());
            match slot.as_ref().filter(|receiver| receiver.has_changed().is_ok()) {
                Some(receiver) => (receiver.clone(), None),
                None => {
                    let (sender, receiver) = watch::channel(None);
                    *slot = Some(receiver.clone());
                    (receiver, Some(sender))
                }
            }
        };
        if let Some(sender) = sender {
            // If the owner is dropped, closing the channel wakes waiters and lets the next
            // call replace the closed slot. No in-flight bit can remain stuck on cancellation.
            let result = refresh.await.map_err(|error| format!("{error:#}"));
            sender.send_replace(Some(result.clone()));
            *self.current.lock().unwrap_or_else(|error| error.into_inner()) = None;
            result.map_err(anyhow::Error::msg)
        } else {
            receiver
                .wait_for(Option::is_some)
                .await
                .map_err(|_| anyhow::anyhow!("刷新任务已中断，请重试"))?
                .as_ref()
                .expect("wait_for requires a completed result")
                .clone()
                .map_err(anyhow::Error::msg)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    #[tokio::test]
    async fn overlapping_calls_share_success_and_failure_then_allow_a_new_batch() {
        for failed in [false, true] {
            let flight = RefreshFlight::default();
            let calls = AtomicUsize::new(0);
            let request = || async {
                calls.fetch_add(1, Ordering::SeqCst);
                tokio::task::yield_now().await;
                if failed {
                    anyhow::bail!("fixture failure");
                }
                Ok(RefreshAllExecutionOutcome {
                    inserted_count: 0,
                    total_count: 0,
                    failed_count: 0,
                    failure_message: None,
                })
            };
            let (first, second) = tokio::join!(flight.run(request()), flight.run(request()));
            assert_eq!(calls.load(Ordering::SeqCst), 1);
            assert_eq!(first.map_err(|e| e.to_string()), second.map_err(|e| e.to_string()));
            let _ = flight.run(request()).await;
            assert_eq!(calls.load(Ordering::SeqCst), 2);
        }
    }

    #[tokio::test]
    async fn cancelled_owner_does_not_leave_refresh_permanently_busy() {
        let flight = Arc::new(RefreshFlight::default());
        let started = Arc::new(tokio::sync::Notify::new());
        let owner = {
            let flight = Arc::clone(&flight);
            let started = Arc::clone(&started);
            tokio::spawn(async move {
                flight
                    .run(async {
                        started.notify_one();
                        std::future::pending().await
                    })
                    .await
            })
        };
        started.notified().await;
        let waiter = flight.run(async { panic!("a joined request must not execute") });
        tokio::pin!(waiter);
        std::future::poll_fn(|cx| {
            assert!(waiter.as_mut().poll(cx).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        owner.abort();
        assert!(owner.await.unwrap_err().is_cancelled());
        let error = tokio::time::timeout(std::time::Duration::from_secs(1), waiter)
            .await
            .expect("waiter must wake on cancellation")
            .unwrap_err();
        assert!(error.to_string().contains("已中断"));
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            flight.run(async {
                Ok(RefreshAllExecutionOutcome {
                    inserted_count: 0,
                    total_count: 0,
                    failed_count: 0,
                    failure_message: None,
                })
            }),
        )
        .await
        .expect("retry must not hang")
        .expect("retry succeeds");
        assert_eq!(result.failure_message, None);
    }
}
