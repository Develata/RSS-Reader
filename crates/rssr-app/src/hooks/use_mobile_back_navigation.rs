use crate::router::AppRoute;
use dioxus_router::Navigator;

#[cfg(target_os = "android")]
use dioxus::mobile::{
    tao::event::{Event as TaoEvent, WindowEvent as TaoWindowEvent},
    use_window, use_wry_event_handler,
};
use dioxus::prelude::*;

mod policy;
use policy::Navigation;
#[cfg(any(target_os = "android", test))]
use policy::{Action, BackPolicy, BackRequest, Event as PolicyEvent};

#[cfg(any(target_os = "android", all(test, not(target_arch = "wasm32"))))]
mod android_bridge;
#[cfg(any(target_os = "android", all(test, not(target_arch = "wasm32"))))]
mod back_capture;
#[cfg(target_os = "android")]
static BACK_CAPTURE: back_capture::BackCaptureGate = back_capture::BackCaptureGate::new();

fn navigation(navigator: Navigator, fallback_route: Option<&AppRoute>) -> Navigation {
    let has_history = navigator.can_go_back();
    Navigation::resolve(
        has_history,
        !has_history
            && fallback_route.is_some_and(|target| history().current_route() != target.to_string()),
    )
}

/// The toolbar back button: the previous page, or `fallback_route` when there is none.
///
/// Synchronous on every platform. The click is a DOM event, and `reading_position.js` records the
/// reading position in its capture phase before the route changes, so there is nothing to wait
/// for. On Android the button used to wait for a further capture round trip like the system key;
/// after returning to a reader through Home it then stayed unresponsive for seconds (the cause of
/// that stall was not pinned down).
pub(crate) fn navigate_back(navigator: Navigator, fallback_route: Option<AppRoute>) -> bool {
    match navigation(navigator, fallback_route.as_ref()) {
        Navigation::History => navigator.go_back(),
        Navigation::Fallback => {
            navigator.replace(fallback_route.expect("resolved a distinct fallback"));
        }
        Navigation::Stay => return false,
    }
    true
}

/// The Android system back key. No DOM event records the reading position, so it is captured first
/// (at most 500 ms); presses while waiting merge, and the request is dropped if the route changed
/// meanwhile.
#[cfg(target_os = "android")]
fn navigate_back_after_capture(navigator: Navigator, fallback_route: Option<AppRoute>) {
    // The policy has already established a target. Re-resolve it after capture,
    // just as toolbar navigation does, provided the route is still current.
    let Some(guard) = BACK_CAPTURE.try_acquire() else {
        return;
    };
    let route = history().current_route();
    spawn(back_capture::finish_capture(
        guard,
        crate::ui::reading_position::capture_current_position(),
        move || history().current_route() == route,
        move || {
            navigate_back(navigator, fallback_route);
        },
    ));
}

#[cfg(any(target_os = "android", test))]
#[derive(Clone, Copy)]
struct BackAdapter {
    policy: Signal<BackPolicy>,
    // Render projection only: decisions never read this signal.
    exit_hint: Signal<bool>,
}

#[cfg(any(target_os = "android", test))]
impl BackAdapter {
    fn handle(mut self, event: PolicyEvent) -> Action {
        let (action, visible) = self.policy.with_mut(|policy| {
            let action = policy.handle(event);
            (action, policy.hint_visible())
        });
        if *self.exit_hint.peek() != visible {
            self.exit_hint.set(visible);
        }
        action
    }
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
        let adapter = BackAdapter { policy: use_signal(BackPolicy::default), exit_hint };
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
                adapter.handle(PolicyEvent::Resumed);
                restore_window_interactivity();
            }
            TaoEvent::WindowEvent { event: TaoWindowEvent::Focused(true), .. } => {
                restore_window_interactivity();
            }
            _ => {}
        });

        let on_back = use_callback(move |()| {
            let request = if dismiss() {
                BackRequest::Consumed
            } else if fallback_route.is_some() {
                BackRequest::Navigate(navigation(navigator, fallback_route.as_ref()))
            } else {
                // Home never follows history, even if the router has one.
                BackRequest::Home
            };
            match adapter.handle(PolicyEvent::Back(request)) {
                Action::None => {}
                Action::Navigate => {
                    navigate_back_after_capture(navigator, fallback_route.clone());
                }
                Action::Background => android_bridge::background_task(),
                Action::ArmHintTimeout(round) => {
                    spawn(async move {
                        tokio::time::sleep(policy::EXIT_CONFIRM_WINDOW).await;
                        adapter.handle(PolicyEvent::HintExpired(round));
                    });
                }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn harness() -> Element {
        let policy = use_signal(BackPolicy::default);
        let exit_hint = use_signal(|| false);
        use_context_provider(|| BackAdapter { policy, exit_hint });
        rsx! {}
    }

    #[test]
    fn adapter_publishes_prompt_state_after_each_event() {
        let mut dom = VirtualDom::new(harness);
        dom.rebuild_in_place();
        dom.in_scope(ScopeId::APP, || {
            let adapter: BackAdapter = consume_context();
            let Action::ArmHintTimeout(old) = adapter.handle(PolicyEvent::Back(BackRequest::Home))
            else {
                panic!("expected first prompt");
            };
            assert!(*adapter.exit_hint.peek());
            adapter.handle(PolicyEvent::Resumed);
            assert!(!*adapter.exit_hint.peek());
            let Action::ArmHintTimeout(current) =
                adapter.handle(PolicyEvent::Back(BackRequest::Home))
            else {
                panic!("expected new prompt after resume");
            };
            adapter.handle(PolicyEvent::HintExpired(old));
            assert!(*adapter.exit_hint.peek());
            adapter.handle(PolicyEvent::HintExpired(current));
            assert!(!*adapter.exit_hint.peek());
            adapter.handle(PolicyEvent::Back(BackRequest::Home));
            assert_eq!(adapter.handle(PolicyEvent::Back(BackRequest::Home)), Action::Background);
            assert!(!*adapter.exit_hint.peek());
        });
    }
}
