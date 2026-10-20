use std::time::Duration;

/// Timer implementation stays at the shell adapter boundary; refresh state and
/// its expiry rule are shared Rust code on every platform.
pub(crate) async fn wait_for_refresh_feedback(duration: Duration) {
    sleep(duration).await;
}

/// Waits `duration` on this platform's timer.
pub(crate) async fn sleep(duration: Duration) {
    #[cfg(target_arch = "wasm32")]
    gloo_timers::future::sleep(duration).await;

    #[cfg(not(target_arch = "wasm32"))]
    tokio::time::sleep(duration).await;
}

/// Leaves the page for the server's sign-in page, which comes back to this path (C4b item 1). The
/// in-process library of the other platforms has no session to end.
pub(crate) fn sign_in_again() {
    #[cfg(target_arch = "wasm32")]
    if let Some(location) = web_sys::window().map(|window| window.location()) {
        let path = location.pathname().unwrap_or_else(|_| "/".into())
            + &location.search().unwrap_or_default();
        let next: String = url::form_urlencoded::byte_serialize(path.as_bytes()).collect();
        let _ = location.set_href(&format!("/login?next={next}"));
    }
}

/// Reloads the page (it is of another build than the server, C4b item 1). Only the browser has
/// one to reload.
pub(crate) fn reload_page() {
    #[cfg(target_arch = "wasm32")]
    if let Some(window) = web_sys::window() {
        let _ = window.location().reload();
    }
}
