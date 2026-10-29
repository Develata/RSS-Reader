//! Admission for preference saves. The main session owns the coalescing queue,
//! recovered-edit suppression and app-wide ordering; no delayed staleness gate lives here.
use super::state::EntriesPageState;

pub(super) fn can_schedule_save(state: &EntriesPageState, captured_loaded: bool) -> bool {
    captured_loaded && state.preferences_load.is_loaded()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pages::entries_page::state::PreferencesLoadState;

    #[test]
    fn scheduling_requires_both_captured_and_live_readiness() {
        let mut state = EntriesPageState::new();
        for readiness in [
            PreferencesLoadState::Pending,
            PreferencesLoadState::Unavailable { message: "failed".into(), tone: "error" },
            PreferencesLoadState::Loaded,
        ] {
            state.preferences_load = readiness;
            assert!(!can_schedule_save(&state, false));
            assert_eq!(can_schedule_save(&state, true), state.preferences_load.is_loaded());
        }
    }
}
