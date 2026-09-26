use crate::router::AppRoute;
use dioxus_router::Navigator;

#[cfg(all(test, not(target_arch = "wasm32"), not(target_os = "android")))]
use dioxus::desktop::tao;
#[cfg(target_os = "android")]
use dioxus::mobile::{
    tao,
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

#[cfg(any(target_os = "android", all(test, not(target_arch = "wasm32"))))]
fn is_back_navigation_key(
    state: tao::event::ElementState,
    repeat: bool,
    physical_key: tao::keyboard::KeyCode,
    logical_key: &tao::keyboard::Key<'_>,
) -> bool {
    use tao::{
        event::ElementState,
        keyboard::{Key, KeyCode},
    };

    state == ElementState::Pressed
        && !repeat
        && matches!(
            (physical_key, logical_key),
            (KeyCode::BrowserBack, _) | (_, Key::BrowserBack | Key::GoBack | Key::Escape)
        )
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
        let window = use_window();

        let restore_window_interactivity = move || {
            let window = window.clone();
            spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(16)).await;
                window.set_visible(true);
                let _ = window.set_focus();
            });
        };

        use_wry_event_handler(move |event, _| {
            let navigate_within_app =
                || dismiss() || navigate_back(navigator, fallback_route.clone());

            match event {
                TaoEvent::WindowEvent {
                    event: TaoWindowEvent::KeyboardInput { event, .. },
                    ..
                } => {
                    if !is_back_navigation_key(
                        event.state,
                        event.repeat,
                        event.physical_key,
                        &event.logical_key,
                    ) {
                        return;
                    }

                    let _ = navigate_within_app();
                }
                TaoEvent::WindowEvent { event: TaoWindowEvent::CloseRequested, .. } => {
                    if !navigate_within_app() {
                        return;
                    }

                    restore_window_interactivity();
                }
                TaoEvent::Resumed => restore_window_interactivity(),
                TaoEvent::WindowEvent { event: TaoWindowEvent::Focused(true), .. } => {
                    restore_window_interactivity();
                }
                _ => {}
            }
        });
    }

    #[cfg(not(target_os = "android"))]
    let _ = (fallback_route, dismiss);
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use tao::{
        event::ElementState,
        keyboard::{Key, KeyCode, NativeKeyCode},
    };

    #[test]
    fn android_browser_back_uses_logical_key_and_fires_only_on_initial_press() {
        // Tao represents Android KEYCODE_BACK as an unidentified physical key,
        // paired with the BrowserBack logical key.
        let physical_key = KeyCode::Unidentified(NativeKeyCode::Android(4));
        assert!(is_back_navigation_key(
            ElementState::Pressed,
            false,
            physical_key,
            &Key::BrowserBack,
        ));
        assert!(!is_back_navigation_key(
            ElementState::Released,
            false,
            physical_key,
            &Key::BrowserBack,
        ));
        assert!(!is_back_navigation_key(
            ElementState::Pressed,
            true,
            physical_key,
            &Key::BrowserBack,
        ));
    }

    #[test]
    fn preserves_existing_back_keys_without_consuming_unrelated_input() {
        let unknown = KeyCode::Unidentified(NativeKeyCode::Unidentified);
        for (physical, logical) in [
            (KeyCode::BrowserBack, Key::Unidentified(NativeKeyCode::Unidentified)),
            (unknown, Key::GoBack),
            (unknown, Key::Escape),
        ] {
            assert!(is_back_navigation_key(ElementState::Pressed, false, physical, &logical));
        }
        for logical in [Key::Enter, Key::Character("r"), Key::BrowserForward] {
            assert!(!is_back_navigation_key(ElementState::Pressed, false, unknown, &logical));
        }
    }
}
