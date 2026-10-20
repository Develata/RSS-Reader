use std::time::Duration;

/// Timer implementation stays at the shell adapter boundary; refresh state and
/// its expiry rule are shared Rust code on every platform.
pub(crate) async fn wait_for_refresh_feedback(duration: Duration) {
    #[cfg(target_arch = "wasm32")]
    gloo_timers::future::sleep(duration).await;

    #[cfg(not(target_arch = "wasm32"))]
    tokio::time::sleep(duration).await;
}
