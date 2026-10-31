//! Result ownership for one Reader visit. The existing reducer owns the entry
//! identity and load generation; the session owns tasks and publishes intents.

use super::{intent::ReaderPageIntent, state::ReaderPageState};

pub(super) struct ReaderResultScope {
    entry_id: i64,
    load_generation: u64,
    reread_generation: Option<u64>,
}

impl ReaderResultScope {
    /// Loads capture after BeginLoading; other commands share the current visit.
    pub(super) fn capture(entry_id: i64, state: &ReaderPageState) -> Self {
        Self { entry_id, load_generation: state.load_generation, reread_generation: None }
    }

    /// Capture after RereadIssued. A later refresh or applied flag write also retires this read.
    pub(super) fn capture_reread(entry_id: i64, state: &ReaderPageState) -> Self {
        Self { reread_generation: Some(state.reread_generation), ..Self::capture(entry_id, state) }
    }

    pub(super) fn is_current(&self, state: &ReaderPageState) -> bool {
        state.current_entry_id == self.entry_id
            && state.load_generation == self.load_generation
            && self.reread_generation.is_none_or(|generation| generation == state.reread_generation)
    }

    pub(super) fn complete<I: IntoIterator<Item = ReaderPageIntent>>(
        self,
        state: &ReaderPageState,
        intents: I,
    ) -> impl Iterator<Item = ReaderPageIntent> + use<I> {
        let current = self.is_current(state);
        // Gate the entire result, including status intents that have no entry ID.
        // Rejected payloads are dropped without traversal; accepted bodies move.
        current.then_some(intents).into_iter().flatten()
    }
}

/// A read-only refresh must not clear a readable body, localize images again on recovery,
/// or race the initial load. The adapter executes the chosen branch in its existing task scope.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum RereadPlan {
    Ignore,
    RecoverLoad,
    AfterLoad,
    ReadFlagsAndNeighbours,
}

pub(super) fn plan_reread(entry_id: i64, state: &ReaderPageState) -> RereadPlan {
    if state.current_entry_id != entry_id {
        RereadPlan::Ignore
    } else if state.error.is_some() {
        RereadPlan::RecoverLoad
    } else if !state.content_loaded {
        RereadPlan::AfterLoad
    } else {
        RereadPlan::ReadFlagsAndNeighbours
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

    #[test]
    fn refresh_admission_follows_load_outcome_without_changing_state() {
        let mut state = ReaderPageState::new();
        assert_eq!(plan_reread(1, &state), RereadPlan::Ignore);
        reduce(&mut state, ReaderPageIntent::BeginLoading { entry_id: 1 });
        assert_eq!(plan_reread(1, &state), RereadPlan::AfterLoad);
        reduce(&mut state, ReaderPageIntent::RereadWanted(true));
        reduce(
            &mut state,
            ReaderPageIntent::SetError { entry_id: 1, error: Some("offline".into()) },
        );
        let failed = state.clone();
        assert_eq!(plan_reread(1, &state), RereadPlan::RecoverLoad);
        assert_eq!(plan_reread(2, &state), RereadPlan::Ignore, "identity precedes the error");
        assert_eq!(state, failed, "the adapter still owns command execution and publication");
        reduce(&mut state, ReaderPageIntent::BeginLoading { entry_id: 1 });
        reduce(&mut state, ReaderPageIntent::SetAssetLocalizationRequested);
        reduce(&mut state, loaded(1, "Recovered"));
        assert_eq!(plan_reread(1, &state), RereadPlan::ReadFlagsAndNeighbours);
        assert!(state.asset_localization_requested, "read-only recovery must not localize again");
        assert!(!state.reread_wanted, "the recovery load consumes deferred refresh demand");
    }

    fn issue_refresh(state: &mut ReaderPageState) -> ReaderResultScope {
        reduce(state, ReaderPageIntent::RereadIssued);
        ReaderResultScope::capture_reread(state.current_entry_id, state)
    }

    #[test]
    fn later_refresh_wins_without_replacing_body_or_localization() {
        let mut state = ReaderPageState::new();
        visit(&mut state, 1, "Current");
        reduce(&mut state, ReaderPageIntent::SetAssetLocalizationRequested);
        let body = Arc::clone(&state.body_text);
        let old = issue_refresh(&mut state);
        let current = issue_refresh(&mut state);
        let reply = current.complete(
            &state,
            [ReaderPageIntent::ApplyReread {
                entry_id: 1,
                is_read: true,
                is_starred: true,
                navigation_state: rssr_domain::EntryNavigation {
                    next_unread_entry_id: Some(3),
                    ..Default::default()
                },
            }],
        );
        apply(&mut state, reply);
        assert!(state.is_read && state.is_starred);
        assert_eq!(state.navigation_state.next_unread_entry_id, Some(3));
        assert!(Arc::ptr_eq(&state.body_text, &body));
        assert!(state.asset_localization_requested);
        let expected = state.clone();
        let reply = old.complete(
            &state,
            [ReaderPageIntent::RereadFailed {
                entry_id: 1,
                failure: crate::ui::failure::plain("old refresh"),
            }],
        );
        apply(&mut state, reply);
        assert_eq!(state, expected, "an obsolete error must not replace current feedback");
    }

    #[test]
    fn applied_flags_retire_refresh_but_not_other_visit_results() {
        // Both flags, even an equal-value acknowledgement, retire an older read.
        for (read, starred) in [(Some(true), None), (None, Some(true)), (Some(false), None)] {
            let mut state = ReaderPageState::new();
            visit(&mut state, 1, "Current");
            let write = ReaderResultScope::capture(1, &state);
            let refresh = issue_refresh(&mut state);
            reduce(
                &mut state,
                ReaderPageIntent::PatchEntryFlags {
                    entry_id: 1,
                    is_read: read,
                    is_starred: starred,
                },
            );
            assert!(!refresh.is_current(&state));
            assert!(
                write.is_current(&state),
                "refresh generations do not retire a visit-owned write"
            );
            let unread_payload = std::iter::from_fn(|| panic!("retired refresh payload was read"));
            assert_eq!(refresh.complete(&state, unread_payload).count(), 0);
            let reply = write.complete(
                &state,
                [ReaderPageIntent::SetStatus {
                    message: "write finished".into(),
                    tone: "success".into(),
                }],
            );
            apply(&mut state, reply);
            assert_eq!(state.status, "write finished");
        }
    }

    #[test]
    fn refresh_identity_does_not_survive_reload_or_return_to_same_article() {
        for next in [1, 2] {
            let mut state = ReaderPageState::new();
            visit(&mut state, 1, "Original");
            let refresh = issue_refresh(&mut state);
            let refresh_generation = state.reread_generation;
            visit(&mut state, next, "Other visit");
            if next != 1 {
                visit(&mut state, 1, "Returned");
            }
            assert_eq!(state.reread_generation, refresh_generation);
            assert!(
                !refresh.is_current(&state),
                "equal refresh generations do not establish ownership"
            );
        }
    }
}
