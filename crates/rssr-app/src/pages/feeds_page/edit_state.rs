//! The single owner of a feed editor's draft and request identity. Rendering,
//! command execution and page-wide feedback remain in the existing adapters.
use rssr_domain::{FeedEditDetails, FeedEditRequest};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FeedEditState {
    feed_id: Option<i64>,
    generation: u64,
    details: Option<FeedEditDetails>,
    busy: bool,
    error: Option<String>,
}

pub(super) struct FeedEditSubmission {
    pub generation: u64,
    pub request: FeedEditRequest,
}

impl FeedEditState {
    pub(super) fn new() -> Self {
        Self { feed_id: None, generation: 0, details: None, busy: false, error: None }
    }

    pub(super) fn feed_id(&self) -> Option<i64> {
        self.feed_id
    }

    pub(super) fn details(&self) -> Option<&FeedEditDetails> {
        self.details.as_ref()
    }

    pub(super) fn is_busy(&self) -> bool {
        self.busy
    }

    pub(super) fn is_saving(&self) -> bool {
        self.busy && self.details.is_some()
    }

    pub(super) fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub(super) fn begin_load(&mut self, feed_id: i64) -> Option<u64> {
        if self.is_saving() {
            return None;
        }
        self.generation = self.generation.wrapping_add(1);
        self.feed_id = Some(feed_id);
        self.details = None;
        self.error = None;
        self.busy = true;
        Some(self.generation)
    }

    pub(super) fn finish_load(
        &mut self,
        feed_id: i64,
        generation: u64,
        result: Result<FeedEditDetails, String>,
    ) {
        // Same-feed reopen and an already accepted draft both reject late reads.
        if self.feed_id == Some(feed_id)
            && self.generation == generation
            && self.busy
            && self.details.is_none()
        {
            self.busy = false;
            match result {
                Ok(details) => self.details = Some(details),
                Err(error) => self.error = Some(error),
            }
        }
    }

    pub(super) fn change_title(&mut self, title: String) {
        if !self.busy
            && let Some(edit) = &mut self.details
        {
            edit.custom_title = (!title.is_empty()).then_some(title);
        }
    }

    pub(super) fn change_url(&mut self, url: String) {
        if !self.busy
            && let Some(edit) = &mut self.details
        {
            edit.url = url;
        }
    }

    pub(super) fn close(&mut self) {
        if !self.is_saving() {
            self.generation = self.generation.wrapping_add(1);
            self.feed_id = None;
            self.details = None;
            self.error = None;
            self.busy = false;
        }
    }

    pub(super) fn begin_save(&mut self) -> Option<FeedEditSubmission> {
        if self.busy {
            return None;
        }
        let edit = self.details.as_ref()?;
        self.busy = true;
        self.error = None;
        Some(FeedEditSubmission {
            generation: self.generation,
            request: FeedEditRequest {
                feed_id: edit.feed_id,
                url: edit.url.clone(),
                custom_title: edit.custom_title.clone(),
            },
        })
    }

    /// True only for an accepted success, which asks the page reducer to reload
    /// and publish its existing success message. Errors retain the draft.
    pub(super) fn finish_save(
        &mut self,
        feed_id: i64,
        generation: u64,
        result: Result<(), String>,
    ) -> bool {
        if self.feed_id != Some(feed_id) || self.generation != generation {
            return false;
        }
        self.busy = false;
        match result {
            Ok(()) => {
                self.feed_id = None;
                self.details = None;
                self.error = None;
                true
            }
            Err(error) => {
                self.error = Some(error);
                false
            }
        }
    }

    #[cfg(test)]
    pub(super) fn generation(&self) -> u64 {
        self.generation
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn details(id: i64) -> FeedEditDetails {
        FeedEditDetails {
            feed_id: id,
            url: "https://example.com/feed".into(),
            original_title: Some("Original".into()),
            custom_title: None,
        }
    }

    fn ready() -> FeedEditState {
        let mut state = FeedEditState::new();
        let generation = state.begin_load(42).unwrap();
        state.finish_load(42, generation, Ok(details(42)));
        state
    }

    #[test]
    fn absent_and_loading_drafts_ignore_edits_and_cannot_save() {
        let mut state = FeedEditState::new();
        for loading in [false, true] {
            if loading {
                state.begin_load(42);
            }
            let before = state.clone();
            state.change_title("ignored".into());
            state.change_url("ignored".into());
            assert!(state.begin_save().is_none());
            assert_eq!(state, before);
        }
        state.close();
        assert_eq!(state.feed_id(), None);
        assert!(!state.is_busy());
    }

    #[test]
    fn reopening_same_feed_rejects_both_late_load_outcomes() {
        for result in [Ok(details(42)), Err("old error".into())] {
            let mut state = FeedEditState::new();
            let old = state.begin_load(42).unwrap();
            state.close();
            let current = state.begin_load(42).unwrap();
            let before = state.clone();
            state.finish_load(42, old, result);
            assert_eq!(state, before);
            state.finish_load(42, current, Ok(details(42)));
            assert!(!state.is_busy());
            assert_eq!(state.details(), Some(&details(42)));
        }
    }

    #[test]
    fn switching_loading_feed_rejects_wrong_identity_and_duplicate_results() {
        let mut state = FeedEditState::new();
        state.begin_load(1);
        let current = state.begin_load(2).unwrap();
        let before = state.clone();
        state.finish_load(1, current, Ok(details(1)));
        assert_eq!(state, before);
        state.finish_load(2, current, Err("read failed".into()));
        let failed = state.clone();
        state.finish_load(2, current, Ok(details(2)));
        assert_eq!(state, failed, "a completed read is accepted at most once");
        assert_eq!(state.error(), Some("read failed"));
        state.begin_load(2);
        assert!(state.error().is_none());
    }

    #[test]
    fn draft_values_are_not_normalized_before_the_application_command() {
        let mut state = ready();
        state.change_title("custom".into());
        state.change_title(String::new());
        assert_eq!(state.details().unwrap().custom_title, None);
        state.change_title("  \t".into());
        state.change_url(" raw url \n".into());
        let submission = state.begin_save().unwrap();
        assert_eq!(submission.request.feed_id, 42);
        assert_eq!(submission.request.custom_title.as_deref(), Some("  \t"));
        assert_eq!(submission.request.url, " raw url \n");
        assert_eq!(state.details().unwrap().original_title.as_deref(), Some("Original"));
    }

    #[test]
    fn saving_blocks_edits_close_reopen_repeat_submit_and_late_reads() {
        let mut state = ready();
        state.change_title("draft".into());
        let submission = state.begin_save().unwrap();
        let saving = state.clone();
        state.change_title("ignored".into());
        state.change_url("ignored".into());
        state.close();
        assert!(state.begin_load(43).is_none());
        assert!(state.begin_save().is_none());
        state.finish_load(42, submission.generation, Ok(details(42)));
        state.finish_load(42, submission.generation, Err("late".into()));
        assert_eq!(state, saving);
        assert!(!state.finish_save(42, submission.generation, Err("save failed".into())));
        assert!(!state.is_busy());
        assert_eq!(state.error(), Some("save failed"));
        assert_eq!(state.details().unwrap().custom_title.as_deref(), Some("draft"));
        state.change_title("retry".into());
        assert_eq!(state.error(), Some("save failed"), "editing alone retains the error");
        let retry = state.begin_save().unwrap();
        assert!(state.error().is_none());
        assert_eq!(retry.generation, submission.generation);
        assert_eq!(retry.request.custom_title.as_deref(), Some("retry"));
    }

    #[test]
    fn save_results_keep_the_existing_identity_only_admission_rule() {
        for result in [Ok(()), Err("failed".into())] {
            let mut state = ready();
            let current = state.generation;
            let before = state.clone();
            assert!(!state.finish_save(43, current, result.clone()));
            assert!(!state.finish_save(42, current.wrapping_sub(1), result));
            assert_eq!(state, before);
        }
        // Preserve the reducer contract: a matching result is not additionally
        // gated on busy, even if it arrives after a previous matching failure.
        let mut state = ready();
        let current = state.generation;
        assert!(!state.finish_save(42, current, Err("failed".into())));
        assert!(state.finish_save(42, current, Ok(())));
    }

    #[test]
    fn successful_save_closes_once_without_allocating_another_generation() {
        let mut state = ready();
        let submission = state.begin_save().unwrap();
        assert!(state.finish_save(42, submission.generation, Ok(())));
        assert_eq!(state.feed_id(), None);
        assert!(state.details().is_none());
        assert!(state.error().is_none());
        assert!(!state.is_busy());
        assert_eq!(state.generation, submission.generation);
        let closed = state.clone();
        assert!(!state.finish_save(42, submission.generation, Ok(())));
        state.finish_load(42, submission.generation, Ok(details(42)));
        assert_eq!(state, closed);
    }

    #[test]
    fn open_and_close_keep_wrapping_generation_even_when_already_closed() {
        let mut state = FeedEditState::new();
        state.generation = u64::MAX;
        assert_eq!(state.begin_load(42), Some(0));
        state.generation = u64::MAX;
        state.close();
        assert_eq!(state.generation, 0);
        state.close();
        assert_eq!(state.generation, 1);
    }
}
