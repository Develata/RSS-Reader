pub(crate) mod external_link;
pub(crate) mod local_time;

#[cfg(not(target_arch = "wasm32"))]
#[path = "bootstrap/native.rs"]
mod imp;

#[cfg(target_arch = "wasm32")]
#[path = "bootstrap/web.rs"]
mod imp;

#[cfg(target_arch = "wasm32")]
pub(crate) use imp::install_history_capture;
pub use imp::{AppServices, ReaderNavigation};

pub(crate) use rssr_api::AddSubscriptionOutcome;
