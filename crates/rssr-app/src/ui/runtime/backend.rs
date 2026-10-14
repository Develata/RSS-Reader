//! The backend the UI runs against: this device's use cases, in process
//! (`rssr_infra::local_backend`).

use std::sync::Arc;

use anyhow::Result;
use rssr_api::Backend;

use crate::bootstrap::AppServices;

/// The backend of this process.
pub(crate) async fn backend() -> Result<Arc<dyn Backend>> {
    Ok(host().await?.backend())
}

async fn host() -> Result<Arc<AppServices>> {
    #[cfg(test)]
    if let Some(host) = test_host::current() {
        return Ok(host);
    }
    AppServices::shared().await
}

/// Runs UI commands of the current test thread against chosen services instead of the
/// process-wide ones (tests run on a current-thread runtime).
#[cfg(test)]
pub(crate) mod test_host {
    use std::cell::RefCell;
    use std::sync::Arc;

    use crate::bootstrap::AppServices;

    thread_local! {
        static HOST: RefCell<Option<Arc<AppServices>>> = const { RefCell::new(None) };
    }

    pub(crate) fn current() -> Option<Arc<AppServices>> {
        HOST.with(|host| host.borrow().clone())
    }

    /// Installs `host` until the returned guard is dropped.
    pub(crate) fn install(host: Arc<AppServices>) -> impl Drop {
        struct Restore(Option<Arc<AppServices>>);
        impl Drop for Restore {
            fn drop(&mut self) {
                let previous = self.0.take();
                HOST.with(|host| *host.borrow_mut() = previous);
            }
        }
        Restore(HOST.with(|current| current.borrow_mut().replace(host)))
    }
}
