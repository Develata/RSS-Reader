use crate::router::AppRoute;

#[cfg(target_os = "android")]
use dioxus::mobile::{
    tao::{
        event::{ElementState, Event as TaoEvent, WindowEvent as TaoWindowEvent},
        keyboard::{Key as TaoKey, KeyCode as TaoKeyCode},
    },
    use_wry_event_handler,
};
#[cfg(target_os = "android")]
use dioxus::prelude::*;

pub fn use_mobile_back_navigation(fallback_route: Option<AppRoute>) {
    #[cfg(target_os = "android")]
    {
        let navigator = use_navigator();
        let history = history();

        use_wry_event_handler(move |event, _| {
            let TaoEvent::WindowEvent {
                event: TaoWindowEvent::KeyboardInput { event, .. }, ..
            } = event
            else {
                return;
            };

            if event.state != ElementState::Pressed
                || event.repeat
                || !matches!(
                    (event.physical_key, &event.logical_key),
                    (TaoKeyCode::BrowserBack, _) | (_, TaoKey::GoBack) | (_, TaoKey::Escape)
                )
            {
                return;
            }

            if history.can_go_back() {
                navigator.go_back();
                return;
            }

            if let Some(target) = fallback_route.clone() {
                let target_path = target.to_string();
                if history.current_route() != target_path {
                    navigator.replace(target);
                }
            }
        });
    }

    #[cfg(not(target_os = "android"))]
    let _ = fallback_route;
}
