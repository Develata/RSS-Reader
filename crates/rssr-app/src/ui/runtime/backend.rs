//! The backend the UI runs against: this device's use cases, in process
//! (`rssr_infra::local_backend`).

use std::sync::Arc;

use anyhow::Result;
use rssr_api::{Backend, BackendError};

use crate::bootstrap::AppServices;

/// The backend of this process. Failing to open it is classified like any backend error.
pub(crate) async fn backend() -> Result<Arc<dyn Backend>, BackendError> {
    #[cfg(test)]
    if let Some(backend) = test_host::current() {
        return Ok(backend);
    }
    Ok(host().await?.backend())
}

async fn host() -> Result<Arc<AppServices>> {
    AppServices::shared().await
}

/// Runs UI commands of the current test thread against a chosen backend instead of the
/// process-wide one (tests run on a current-thread runtime): the library in memory, or a
/// [`super::fake_backend::Scripted`] one over it (C4b).
#[cfg(test)]
pub(crate) mod test_host {
    use std::cell::RefCell;
    use std::sync::Arc;

    use rssr_api::Backend;

    thread_local! {
        static HOST: RefCell<Option<Arc<dyn Backend>>> = const { RefCell::new(None) };
    }

    pub(crate) fn current() -> Option<Arc<dyn Backend>> {
        HOST.with(|host| host.borrow().clone())
    }

    /// Installs `host` until the returned guard is dropped.
    pub(crate) fn install(host: Arc<dyn Backend>) -> impl Drop {
        struct Restore(Option<Arc<dyn Backend>>);
        impl Drop for Restore {
            fn drop(&mut self) {
                let previous = self.0.take();
                HOST.with(|host| *host.borrow_mut() = previous);
            }
        }
        Restore(HOST.with(|current| current.borrow_mut().replace(host)))
    }
}
