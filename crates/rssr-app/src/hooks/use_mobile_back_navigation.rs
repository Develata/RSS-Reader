use crate::router::AppRoute;
use dioxus_router::Navigator;

#[cfg(target_os = "android")]
use dioxus::mobile::{
    tao::event::{Event as TaoEvent, WindowEvent as TaoWindowEvent},
    use_window, use_wry_event_handler,
};
use dioxus::prelude::*;

#[cfg(any(target_os = "android", all(test, not(target_arch = "wasm32"))))]
mod android_bridge;
#[cfg(any(target_os = "android", all(test, not(target_arch = "wasm32"))))]
mod back_capture;
#[cfg(target_os = "android")]
static BACK_CAPTURE: back_capture::BackCaptureGate = back_capture::BackCaptureGate::new();

/// 首页上第一次按返回后，等第二次按下的时长；过了就收起提示，重新计。
#[cfg(target_os = "android")]
const EXIT_CONFIRM_WINDOW: std::time::Duration = std::time::Duration::from_secs(5);

/// The toolbar back button: the previous page, or `fallback_route` when there is none.
///
/// Synchronous on every platform. The click is a DOM event, and `reading_position.js` records the
/// reading position in its capture phase before the route changes, so there is nothing to wait
/// for. On Android the button used to wait for a further capture round trip like the system key;
/// after returning to a reader through Home it then stayed unresponsive for seconds (the cause of
/// that stall was not pinned down).
pub(crate) fn navigate_back(navigator: Navigator, fallback_route: Option<AppRoute>) -> bool {
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

/// The Android system back key. No DOM event records the reading position, so it is captured first
/// (at most 500 ms); presses while waiting merge, and the request is dropped if the route changed
/// meanwhile.
#[cfg(target_os = "android")]
fn navigate_back_after_capture(navigator: Navigator, fallback_route: Option<AppRoute>) -> bool {
    if !navigator.can_go_back()
        && !fallback_route
            .as_ref()
            .is_some_and(|target| history().current_route() != target.to_string())
    {
        return false;
    }
    let Some(guard) = BACK_CAPTURE.try_acquire() else {
        return true;
    };
    let route = history().current_route();
    spawn(async move {
        let _guard = guard;
        crate::ui::reading_position::capture_current_position().await;
        if history().current_route() == route {
            navigate_back(navigator, fallback_route);
        }
    });
    true
}

/// 页面上的 Android 系统返回键：回到上一页，没有上一页就去 `fallback_route`。
///
/// `fallback_route` 为 `None` 的是首页：第一次按返回只显示“再按一次退出”（返回的信号为真），
/// 5 秒内仍在首页再按一次，应用退到后台；首页不沿历史后退。其他平台上不做任何事，信号恒为假。
pub fn use_mobile_back_navigation(fallback_route: Option<AppRoute>) -> Signal<bool> {
    use_back_navigation(fallback_route, || false)
}

/// Reader overlays get first refusal, without adding a second native navigation path.
pub(crate) fn use_mobile_back_navigation_with_dismiss(
    fallback_route: Option<AppRoute>,
    dismiss: impl Fn() -> bool + 'static,
) -> Signal<bool> {
    use_back_navigation(fallback_route, dismiss)
}

fn use_back_navigation(
    fallback_route: Option<AppRoute>,
    dismiss: impl Fn() -> bool + 'static,
) -> Signal<bool> {
    let exit_hint = use_signal(|| false);

    #[cfg(target_os = "android")]
    {
        let mut exit_hint = exit_hint;
        // 每次显示提示加一；到时只收起自己那一次的提示。
        let mut hint_round = use_signal(|| 0_u64);
        let navigator = use_navigator();
        let window = use_window();

        let restore_window_interactivity = move || {
            let window = window.clone();
            spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(16)).await;
                window.set_visible(true);
                let _ = window.set_focus();
            });
        };

        // tao 在 Android 上不送按键与关窗事件（输入队列未接通），系统返回键由 MainActivity
        // 经 JNI 转来，见下方 `android`；这里只在回到前台时恢复窗口。
        use_wry_event_handler(move |event, _| match event {
            TaoEvent::Resumed => {
                // 退到后台再回来，之前那次“再按一次”不再算数。
                exit_hint.set(false);
                restore_window_interactivity();
            }
            TaoEvent::WindowEvent { event: TaoWindowEvent::Focused(true), .. } => {
                restore_window_interactivity();
            }
            _ => {}
        });

        let on_back = use_callback(move |()| {
            if dismiss() {
                return;
            }
            if fallback_route.is_some() {
                navigate_back_after_capture(navigator, fallback_route.clone());
            } else if exit_hint() {
                exit_hint.set(false);
                android_bridge::background_task();
            } else {
                exit_hint.set(true);
                let round = hint_round() + 1;
                hint_round.set(round);
                spawn(async move {
                    tokio::time::sleep(EXIT_CONFIRM_WINDOW).await;
                    if hint_round() == round {
                        exit_hint.set(false);
                    }
                });
            }
        });
        let registration = use_hook(move || {
            let (sender, mut receiver) = tokio::sync::mpsc::channel(1);
            let registration = android_bridge::register(sender);
            spawn(async move {
                while receiver.recv().await.is_some() {
                    on_back.call(());
                }
            });
            registration
        });
        use_drop(move || android_bridge::unregister(registration));
    }

    #[cfg(not(target_os = "android"))]
    let _ = (fallback_route, dismiss);

    exit_hint
}
