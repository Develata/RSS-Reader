#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SettingsPageSaveState {
    pub(crate) pending_save: bool,
}

impl SettingsPageSaveState {
    pub(crate) fn new() -> Self {
        Self { pending_save: false }
    }
}
