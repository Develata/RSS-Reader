//! Result ownership for one Reader visit. The existing reducer owns the entry
//! identity and load generation; the session owns tasks and publishes intents.

use super::{intent::ReaderPageIntent, state::ReaderPageState};

pub(super) struct ReaderResultScope {
    entry_id: i64,
    load_generation: u64,
}

impl ReaderResultScope {
    /// Loads capture after BeginLoading; other commands share the current visit.
    pub(super) fn capture(entry_id: i64, state: &ReaderPageState) -> Self {
        Self { entry_id, load_generation: state.load_generation }
    }

    pub(super) fn complete<I: IntoIterator<Item = ReaderPageIntent>>(
        self,
        state: &ReaderPageState,
        intents: I,
    ) -> impl Iterator<Item = ReaderPageIntent> + use<I> {
        let current = state.current_entry_id == self.entry_id
            && state.load_generation == self.load_generation;
        // Gate the entire result, including status intents that have no entry ID.
        // Rejected payloads are dropped without traversal; accepted bodies move.
        current.then_some(intents).into_iter().flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pages::reader_page::{
        reducer::reduce_reader_page_intent as reduce, state::ReaderPageLoadedContent,
    };
    use std::sync::Arc;

    fn loaded(entry_id: i64, title: &str) -> ReaderPageIntent {
        ReaderPageIntent::ApplyLoadedContent {
            entry_id,
            content: ReaderPageLoadedContent {
                title: title.into(),
                body_text: format!("{title} body"),
                body_html: Some(format!("<p>{title}</p>")),
                source: "Feed".into(),
                author: None,
                original_url: None,
                published_at: "Unknown".into(),
                navigation_state: Default::default(),
                is_read: false,
                is_starred: false,
            },
        }
    }

    fn visit(state: &mut ReaderPageState, entry_id: i64, title: &str) -> ReaderResultScope {
        reduce(state, ReaderPageIntent::BeginLoading { entry_id });
        let scope = ReaderResultScope::capture(entry_id, state);
        reduce(state, loaded(entry_id, title));
        scope
    }

    fn apply(state: &mut ReaderPageState, intents: impl Iterator<Item = ReaderPageIntent>) {
        for intent in intents {
            reduce(state, intent);
        }
    }

    #[test]
    fn accepted_load_preserves_intent_order_and_moves_the_body() {
        let mut state = ReaderPageState::new();
        reduce(&mut state, ReaderPageIntent::BeginLoading { entry_id: 1 });
        let scope = ReaderResultScope::capture(1, &state);
        let result = loaded(1, "Current");
        let ReaderPageIntent::ApplyLoadedContent { content, .. } = &result else { unreachable!() };
        let body_pointer = content.body_text.as_ptr();
        let html_pointer = content.body_html.as_ref().unwrap().as_ptr();
        let mut intents = scope.complete(
            &state,
            [
                result,
                ReaderPageIntent::PatchEntryFlags {
                    entry_id: 1,
                    is_read: Some(true),
                    is_starred: None,
                },
                ReaderPageIntent::SetAssetLocalizationRequested,
                ReaderPageIntent::SetStatus { message: "ready".into(), tone: "info".into() },
            ],
        );
        let first = intents.next().unwrap();
        let ReaderPageIntent::ApplyLoadedContent { content, .. } = &first else { unreachable!() };
        assert_eq!(content.body_text.as_ptr(), body_pointer);
        assert_eq!(content.body_html.as_ref().unwrap().as_ptr(), html_pointer);
        reduce(&mut state, first);
        apply(&mut state, intents);
        assert!(state.content_loaded);
        assert_eq!(state.title, "Current");
        assert!(state.is_read, "the flag patch follows the loaded content");
        assert!(state.asset_localization_requested);
        assert_eq!(state.status, "ready");
    }

    #[test]
    fn changing_article_rejects_the_whole_batch_including_unscoped_status() {
        let mut state = ReaderPageState::new();
        let old = visit(&mut state, 1, "A");
        visit(&mut state, 2, "B");
        let before = state.clone();
        let intents = old.complete(
            &state,
            [
                ReaderPageIntent::SetStatus { message: "old".into(), tone: "error".into() },
                loaded(1, "Old A"),
                ReaderPageIntent::SetAssetLocalizationRequested,
                ReaderPageIntent::PatchEntryFlags {
                    entry_id: 1,
                    is_read: Some(true),
                    is_starred: Some(true),
                },
                ReaderPageIntent::SetError { entry_id: 1, error: Some("old error".into()) },
            ],
        );
        apply(&mut state, intents);
        assert_eq!(state, before);
    }

    #[test]
    fn returning_to_an_article_does_not_revive_its_original_visit() {
        let mut state = ReaderPageState::new();
        let old = visit(&mut state, 1, "Original A");
        visit(&mut state, 2, "B");
        visit(&mut state, 1, "Latest A");
        let before = state.clone();
        let intents = old.complete(&state, [loaded(1, "Original A")]);
        apply(&mut state, intents);
        assert_eq!(state, before);
    }

    #[test]
    fn same_article_reload_retires_previous_success_and_failure() {
        for result in [
            loaded(1, "Old content"),
            ReaderPageIntent::SetError { entry_id: 1, error: Some("old failure".into()) },
        ] {
            let mut state = ReaderPageState::new();
            let old = visit(&mut state, 1, "Original");
            let current = visit(&mut state, 1, "Retry");
            let before = state.clone();
            let intents = old.complete(&state, [result]);
            apply(&mut state, intents);
            assert_eq!(state, before);
            let intents = current.complete(&state, [loaded(1, "Current success")]);
            apply(&mut state, intents);
            assert_eq!(state.title, "Current success");
        }
    }

    #[test]
    fn command_identity_is_not_replaced_by_the_current_state_identity() {
        let mut state = ReaderPageState::new();
        visit(&mut state, 2, "B");
        let old_command = ReaderResultScope::capture(1, &state);
        let before = state.clone();
        let intents = old_command.complete(
            &state,
            [ReaderPageIntent::SetStatus { message: "belongs to A".into(), tone: "info".into() }],
        );
        apply(&mut state, intents);
        assert_eq!(state, before, "equal generations alone do not establish ownership");
    }

    #[test]
    fn current_commands_share_the_visit_and_preserve_body_allocation() {
        let mut state = ReaderPageState::new();
        visit(&mut state, 1, "Current");
        let read = ReaderResultScope::capture(1, &state);
        let star = ReaderResultScope::capture(1, &state);
        let body = Arc::clone(&state.body_text);
        let html = Arc::clone(state.body_html.as_ref().unwrap());
        for (scope, is_read, is_starred) in [(read, Some(true), None), (star, None, Some(true))] {
            let intents = scope.complete(
                &state,
                [ReaderPageIntent::PatchEntryFlags { entry_id: 1, is_read, is_starred }],
            );
            apply(&mut state, intents);
        }
        assert!(state.is_read && state.is_starred);
        assert!(Arc::ptr_eq(&state.body_text, &body));
        assert!(Arc::ptr_eq(state.body_html.as_ref().unwrap(), &html));
        assert_eq!(state.load_generation, 1);
    }

    #[test]
    fn rejected_payload_is_not_traversed() {
        let mut state = ReaderPageState::new();
        let old = visit(&mut state, 1, "Original");
        visit(&mut state, 1, "Retry");
        let payload = std::iter::from_fn(|| panic!("retired result payload was read"));
        assert_eq!(old.complete(&state, payload).count(), 0);
    }

    #[test]
    fn current_command_failure_is_published_without_reloading_content() {
        let mut state = ReaderPageState::new();
        visit(&mut state, 1, "Current");
        let scope = ReaderResultScope::capture(1, &state);
        let body = Arc::clone(&state.body_text);
        let intents = scope.complete(
            &state,
            [ReaderPageIntent::SetStatus { message: "write failed".into(), tone: "error".into() }],
        );
        apply(&mut state, intents);
        assert!(state.content_loaded);
        assert_eq!(state.status, "write failed");
        assert_eq!(state.status_tone, "error");
        assert!(state.error.is_none());
        assert!(Arc::ptr_eq(&state.body_text, &body));
    }
}
