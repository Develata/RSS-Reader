use crate::router::AppRoute;
use dioxus_router::Navigator;

#[cfg(target_os = "android")]
use dioxus::mobile::{
    tao::event::{Event as TaoEvent, WindowEvent as TaoWindowEvent},
    use_window, use_wry_event_handler,
};
use dioxus::prelude::*;

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

/// 首页上第一次按返回后，等第二次按下的时长；过了就收起提示，重新计。
#[cfg(target_os = "android")]
const EXIT_CONFIRM_WINDOW: std::time::Duration = std::time::Duration::from_secs(5);

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
) {
    use_back_navigation(fallback_route, dismiss);
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
                navigate_back(navigator, fallback_route.clone());
            } else if exit_hint() {
                exit_hint.set(false);
                android::move_task_to_back();
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
        use_future(move || async move {
            loop {
                android::BACK_REQUESTS.notified().await;
                on_back.call(());
            }
        });
    }

    #[cfg(not(target_os = "android"))]
    let _ = (fallback_route, dismiss);

    exit_hint
}

#[cfg(target_os = "android")]
mod android {
    use std::{
        panic::{AssertUnwindSafe, catch_unwind},
        sync::LazyLock,
    };

    use anyhow::Context;
    use jni::{JNIEnv, JavaVM, objects::JObject};
    use tokio::sync::Notify;

    /// 系统返回键。MainActivity 不让 WebView 沿它自己的历史后退（应用内跳转不进那份历史，
    /// 它只记页内锚点，后退只会无声地弹掉锚点），而是经 JNI 通知这里，由当前页面处理。
    /// 只唤醒已经在等的页面：换页途中按下的那一次丢掉，不会留到下一页才生效。
    pub(super) static BACK_REQUESTS: LazyLock<Notify> = LazyLock::new(Notify::new);

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_dev_dioxus_main_MainActivity_requestAppBack(
        _env: JNIEnv,
        _activity: JObject,
    ) {
        if catch_unwind(AssertUnwindSafe(|| BACK_REQUESTS.notify_waiters())).is_err() {
            tracing::error!("Android 返回键 JNI 回调发生 panic，已阻止其跨越 FFI 边界");
        }
    }

    /// 首页上确认退出：Activity 退到后台而不结束，与 Android 12 起根 Activity 的默认返回
    /// 一致，也不会在下次打开时再起一个应用实例。
    pub(super) fn move_task_to_back() {
        if let Err(error) = call_exit_to_background() {
            tracing::error!("无法让应用退到后台：{error:#}");
        }
    }

    fn call_exit_to_background() -> anyhow::Result<()> {
        let context = ndk_context::android_context();
        let vm =
            unsafe { JavaVM::from_raw(context.vm().cast()) }.context("无法访问 Android JavaVM")?;
        let mut env =
            vm.attach_current_thread().context("无法把返回键处理线程附加到 Android JavaVM")?;
        let activity = unsafe { JObject::from_raw(context.context().cast()) };
        env.call_method(&activity, "exitToBackground", "()V", &[])
            .context("无法调用 MainActivity.exitToBackground")?;
        Ok(())
    }
}
