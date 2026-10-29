//! Stored settings, per-field overlays and the page write/read fence. No platform I/O.
use rssr_application::SettingsPatch;
use rssr_domain::UserSettings;

/// The user's edits that are not stored yet.
#[derive(Clone, Default, PartialEq)]
pub(super) struct Edits {
    /// Sent by the save in flight: stored by its reply, or edits again if it fails.
    pub(super) submitted: Option<SettingsPatch>,
    /// Made since (all of them while no save is in flight).
    pub(super) pending: SettingsPatch,
}

impl Edits {
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

/// The page's writes (saves, pulls) as its loads see them.
#[derive(Clone, Default, PartialEq)]
pub(super) struct Writes {
    /// One write at a time: a pull replaces the stored settings a save in flight is measured
    /// against, and a save would store over a pull it does not know of.
    pub(super) in_flight: bool,
    /// Writes whose reply carried the stored settings. A load issued before one of them may
    /// predate it, so its reply is discarded.
    pub(super) stored: u64,
    /// A load answered while a write was in flight, with the app's count of applied settings when
    /// it was issued and its read generation. It waits for the write: dropped if the write stores
    /// settings or a later load was issued meanwhile (review C4B-B-R2-02), applied if not.
    pub(super) held_load: Option<(UserSettings, u64, u64)>,
    /// Loads issued: only the latest one's reply applies, whatever order the replies come in
    /// (C4b item 2; review C4B-B-01).
    pub(super) reads: u64,
}
