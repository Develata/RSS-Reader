use crate::router::AppRoute;
use dioxus_router::Navigator;

use dioxus::prelude::*;

#[cfg(any(target_os = "android", all(test, not(target_arch = "wasm32"))))]
mod android_bridge;

#[cfg(target_os = "android")]
thread_local! {
    static BACK_CAPTURE_PENDING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
#[cfg(target_os = "android")]
struct BackCaptureGuard;
#[cfg(target_os = "android")]
impl Drop for BackCaptureGuard {
    fn drop(&mut self) {
        BACK_CAPTURE_PENDING.set(false);
    }
}

/// The toolbar and native back key share history/fallback policy.
pub(crate) fn navigate_back(navigator: Navigator, fallback_route: Option<AppRoute>) -> bool {
    #[cfg(target_os = "android")]
    {
        if !navigator.can_go_back()
            && !fallback_route
                .as_ref()
                .is_some_and(|target| history().current_route() != target.to_string())
        {
            return false;
        }
        if BACK_CAPTURE_PENDING.replace(true) {
            return true;
        }
        let guard = BackCaptureGuard;
        let route = history().current_route();
        spawn(async move {
            let _guard = guard;
            crate::ui::reading_position::capture_current_position().await;
            if history().current_route() == route {
                finish_back_navigation(navigator, fallback_route);
            }
        });
        true
    }
    #[cfg(not(target_os = "android"))]
    finish_back_navigation(navigator, fallback_route)
}

fn finish_back_navigation(navigator: Navigator, fallback_route: Option<AppRoute>) -> bool {
    if navigator.can_go_back() {
        navigator.go_back();
        return true;
    }
    if let Some(target) = fallback_route
        && history().current_route() != target.to_string()
    {
        navigator.replace(target);
        return true;
    }
    false
}

pub fn use_mobile_back_navigation(fallback_route: Option<AppRoute>) {
    use_mobile_back_navigation_with_dismiss(fallback_route, || false);
}

/// Reader overlays get first refusal, without adding a second native navigation path.
pub(crate) fn use_mobile_back_navigation_with_dismiss(
    fallback_route: Option<AppRoute>,
    dismiss: impl Fn() -> bool + 'static,
) {
    #[cfg(target_os = "android")]
    {
        let navigator = use_navigator();
        let registration = use_hook(move || {
            let (sender, mut receiver) = tokio::sync::mpsc::channel(1);
            let registration = android_bridge::register(sender);
            // This task belongs to the page scope and is cancelled on unmount.
            // JNI never accesses Dioxus signals or navigation from the Java thread.
            spawn(async move {
                while receiver.recv().await.is_some() {
                    if !dismiss() && !navigate_back(navigator, fallback_route.clone()) {
                        android_bridge::background_task();
                    }
                }
            });
            registration
        });
        use_drop(move || android_bridge::unregister(registration));
    }

    #[cfg(not(target_os = "android"))]
    let _ = (fallback_route, dismiss);
}
