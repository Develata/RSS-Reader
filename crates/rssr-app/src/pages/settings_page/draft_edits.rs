//! Per-field draft ownership, submission and failure recovery. No signals or I/O.
use rssr_application::SettingsPatch;
use rssr_domain::UserSettings;

/// The user's edits that are not stored yet.
#[derive(Clone, Default, PartialEq)]
pub(super) struct Edits {
    /// Sent by the save in flight: stored by its reply, or edits again if it fails.
    submitted: Option<SettingsPatch>,
    /// Made since (all of them while no save is in flight).
    pending: SettingsPatch,
}

impl Edits {
    pub(super) fn restored(pending: SettingsPatch) -> Self {
        Self { submitted: None, pending }
    }

    pub(super) fn has_unsaved_changes(&self) -> bool {
        !self.pending.is_empty() || self.submitted.is_some()
    }

    pub(super) fn pending(&self) -> &SettingsPatch {
        &self.pending
    }

    pub(super) fn record(&mut self, changed: SettingsPatch) {
        self.pending = std::mem::take(&mut self.pending).followed_by(changed);
    }

    /// Called only after the session admits a save. The returned patch can be
    /// normalized for storage; the submitted overlay keeps the user's input.
    pub(super) fn submit(&mut self) -> SettingsPatch {
        let submitted = std::mem::take(&mut self.pending);
        self.submitted = Some(submitted.clone());
        submitted
    }

    pub(super) fn saved(&mut self) {
        self.submitted = None;
    }

    pub(super) fn failed(&mut self) {
        if let Some(submitted) = self.submitted.take() {
            self.pending = submitted.followed_by(std::mem::take(&mut self.pending));
        }
    }

    pub(super) fn over(&self, stored: &UserSettings) -> UserSettings {
        let submitted = self.submitted.as_ref().map(|patch| patch.applied(stored));
        self.pending.applied(submitted.as_ref().unwrap_or(stored))
    }

    pub(super) fn numeric_edits(&self) -> [bool; 4] {
        let fields = |patch: &SettingsPatch| {
            [
                patch.refresh_interval_minutes.is_some(),
                patch.archive_after_months.is_some(),
                patch.entries_page_size.is_some(),
                patch.reader_font_scale.is_some(),
            ]
        };
        let pending = fields(&self.pending);
        let submitted = self.submitted.as_ref().map(fields).unwrap_or([false; 4]);
        std::array::from_fn(|i| pending[i] || submitted[i])
    }

    pub(super) fn edit_custom_css(&self) -> bool {
        self.pending.custom_css.is_some()
            || self.submitted.as_ref().is_some_and(|patch| patch.custom_css.is_some())
    }

    /// All of them: the ones a save in flight sent, then the ones made since (C4b item 4).
    pub(super) fn unsaved(&self) -> Option<SettingsPatch> {
        let all = match &self.submitted {
            Some(submitted) => submitted.clone().followed_by(self.pending.clone()),
            None => self.pending.clone(),
        };
        (!all.is_empty()).then_some(all)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page_size(value: u32) -> SettingsPatch {
        SettingsPatch { entries_page_size: Some(value), ..Default::default() }
    }

    #[test]
    fn reverting_a_field_remains_an_edit_over_new_stored_settings() {
        let initial = UserSettings::default();
        let mut edits = Edits::default();
        edits.record(page_size(80));
        edits.record(page_size(initial.entries_page_size));
        let stored = UserSettings {
            entries_page_size: 120,
            refresh_interval_minutes: 90,
            ..initial.clone()
        };
        assert_eq!(edits.over(&stored).entries_page_size, initial.entries_page_size);
        assert_eq!(edits.over(&stored).refresh_interval_minutes, 90);
        assert_eq!(edits.unsaved(), Some(page_size(initial.entries_page_size)));
    }

    #[test]
    fn failure_restores_submitted_fields_below_later_edits_for_retry_and_recovery() {
        let mut edits = Edits::restored(SettingsPatch {
            entries_page_size: Some(80),
            custom_css: Some("/* original */".into()),
            ..Default::default()
        });
        let original = edits.submit();
        edits.record(SettingsPatch {
            entries_page_size: Some(100),
            refresh_interval_minutes: Some(90),
            ..Default::default()
        });
        let expected = original.followed_by(edits.pending().clone());
        assert_eq!(edits.unsaved(), Some(expected.clone()), "reload recovery during save");
        edits.failed();
        assert_eq!(edits.submit(), expected, "explicit retry keeps both sets of edits");
    }

    #[test]
    fn success_retires_only_submitted_fields_and_keeps_later_numeric_ownership() {
        let mut edits = Edits::restored(SettingsPatch {
            entries_page_size: Some(0),
            custom_css: Some("/* submitted */".into()),
            ..Default::default()
        });
        let mut saved = edits.submit().applied(&UserSettings::default());
        saved.entries_page_size = 100;
        edits.record(SettingsPatch { refresh_interval_minutes: Some(75), ..Default::default() });
        assert_eq!(edits.numeric_edits(), [true, false, true, false]);
        assert!(edits.edit_custom_css());
        edits.saved();
        assert_eq!(edits.numeric_edits(), [true, false, false, false]);
        assert!(!edits.edit_custom_css());
        assert_eq!(edits.over(&saved).entries_page_size, 100);
        assert_eq!(edits.over(&saved).refresh_interval_minutes, 75);
        assert_eq!(edits.unsaved(), Some(edits.pending().clone()));
    }
}
