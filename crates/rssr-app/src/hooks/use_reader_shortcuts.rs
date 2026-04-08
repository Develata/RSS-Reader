use dioxus::prelude::*;

use crate::{pages::reader_page::ReaderPageSession, router::AppRoute};

pub fn use_reader_shortcuts(session: ReaderPageSession) -> Callback<KeyboardEvent> {
    let navigator = use_navigator();

    use_callback(move |event: KeyboardEvent| {
        let key = event.key().to_string().to_lowercase();

        match key.as_str() {
            "arrowleft" => {
                if let Some(target) = session.previous_action_target() {
                    navigator.push(AppRoute::ReaderPage { entry_id: target });
                }
            }
            "arrowright" => {
                if let Some(target) = session.next_action_target() {
                    navigator.push(AppRoute::ReaderPage { entry_id: target });
                }
            }
            "m" => session.toggle_read(true),
            "f" => session.toggle_starred(true),
            _ => {}
        }
    })
}
