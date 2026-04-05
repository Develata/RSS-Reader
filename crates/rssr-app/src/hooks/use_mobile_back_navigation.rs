use crate::router::AppRoute;

#[cfg(target_os = "android")]
use dioxus::mobile::{
    tao::{
        event::{ElementState, Event as TaoEvent, WindowEvent as TaoWindowEvent},
        keyboard::{Key as TaoKey, KeyCode as TaoKeyCode},
    },
    use_window, use_wry_event_handler,
};
#[cfg(target_os = "android")]
use dioxus::prelude::*;

pub fn use_mobile_back_navigation(fallback_route: Option<AppRoute>) {
    #[cfg(target_os = "android")]
    {
        let navigator = use_navigator();
        let history = history();
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
            let navigate_within_app = || {
                if history.can_go_back() {
                    navigator.go_back();
                    return true;
                }

                if let Some(target) = fallback_route.clone() {
                    let target_path = target.to_string();
                    if history.current_route() != target_path {
                        navigator.replace(target);
                        return true;
                    }
                }

                false
            };

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
    let _ = fallback_route;
}
