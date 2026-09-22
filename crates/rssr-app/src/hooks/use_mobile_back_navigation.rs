use crate::router::AppRoute;
use dioxus_router::Navigator;

#[cfg(target_os = "android")]
use dioxus::mobile::{
    tao::{
        event::{ElementState, Event as TaoEvent, WindowEvent as TaoWindowEvent},
        keyboard::{Key as TaoKey, KeyCode as TaoKeyCode},
    },
    use_window, use_wry_event_handler,
};
use dioxus::prelude::*;

/// The toolbar and native back key share history/fallback policy.
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
                    if event.state != ElementState::Pressed
                        || event.repeat
                        || !matches!(
                            (event.physical_key, &event.logical_key),
                            (TaoKeyCode::BrowserBack, _)
                                | (_, TaoKey::GoBack)
                                | (_, TaoKey::Escape)
                        )
                    {
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
