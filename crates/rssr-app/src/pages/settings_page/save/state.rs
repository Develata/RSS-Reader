/// Render snapshot only; the mutable write gate lives in the page request lifecycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SettingsPageSaveState {
    pub(crate) pending_save: bool,
}
