use dioxus::prelude::*;

use crate::{
    bootstrap::ReaderNavigation,
    pages::reader_page::{
        ReaderPageBindings, ReaderPageEffect, ReaderPageState, execute_reader_page_effect,
    },
    router::AppRoute,
};

pub fn use_reader_shortcuts(
    entry_id: i64,
    state: Signal<ReaderPageState>,
    bindings: ReaderPageBindings,
) -> Callback<KeyboardEvent> {
    let navigator = use_navigator();

    use_callback(move |event: KeyboardEvent| {
        let key = event.key().to_string().to_lowercase();

        match key.as_str() {
            "arrowleft" => {
                if let Some(target) = previous_action_target(state().navigation_state) {
                    navigator.push(AppRoute::ReaderPage { entry_id: target });
                }
            }
            "arrowright" => {
                if let Some(target) = next_action_target(state().navigation_state) {
                    navigator.push(AppRoute::ReaderPage { entry_id: target });
                }
            }
            "m" => {
                spawn(async move {
                    let outcome = execute_reader_page_effect(ReaderPageEffect::ToggleRead {
                        entry_id,
                        currently_read: state().is_read,
                        via_shortcut: true,
                    })
                    .await;
                    bindings.apply_runtime_outcome(outcome);
                });
            }
            "f" => {
                spawn(async move {
                    let outcome = execute_reader_page_effect(ReaderPageEffect::ToggleStarred {
                        entry_id,
                        currently_starred: state().is_starred,
                        via_shortcut: true,
                    })
                    .await;
                    bindings.apply_runtime_outcome(outcome);
                });
            }
            _ => {}
        }
    })
}

fn previous_action_target(navigation: ReaderNavigation) -> Option<i64> {
    navigation.previous_unread_entry_id.or(navigation.previous_feed_entry_id)
}

fn next_action_target(navigation: ReaderNavigation) -> Option<i64> {
    navigation.next_unread_entry_id.or(navigation.next_feed_entry_id)
}
