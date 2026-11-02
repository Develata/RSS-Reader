pub(crate) mod external_link;
pub(crate) mod local_time;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod native_document;

#[cfg(not(target_arch = "wasm32"))]
#[path = "bootstrap/native.rs"]
mod imp;

#[cfg(target_arch = "wasm32")]
#[path = "bootstrap/web.rs"]
mod imp;

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
pub(crate) use imp::close_local_library;
#[cfg(target_arch = "wasm32")]
pub(crate) use imp::install_history_capture;
pub use imp::{AppServices, ReaderNavigation};

/// WebView settings the app root applies once; a no-op where the host needs none.
pub(crate) fn use_host_webview_settings() {
    #[cfg(target_os = "linux")]
    native_document::use_direct_wheel_scrolling();
}

pub(crate) use rssr_application::AddSubscriptionOutcome;
