use rssr_domain::UserSettings;

use crate::ui::failure::UiFailure;

#[derive(Debug, Clone)]
pub(crate) enum SettingsPageIntent {
    SettingsLoaded(UserSettings),
    SetStatus {
        message: String,
        tone: String,
    },
    /// A backend call failed (C4b item 1).
    Failed(UiFailure),
}

impl SettingsPageIntent {
    /// The backend failure this intent carries.
    pub(crate) fn failure(&self) -> Option<&UiFailure> {
        match self {
            Self::Failed(failure) => Some(failure),
            _ => None,
        }
    }
}
