#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SettingsPageSyncState {
    pub(crate) endpoint: String,
    pub(crate) remote_path: String,
    pub(crate) pending_remote_pull: bool,
}

impl SettingsPageSyncState {
    pub(crate) fn new() -> Self {
        Self {
            endpoint: String::new(),
            remote_path: "config/rss-reader.json".to_string(),
            pending_remote_pull: false,
        }
    }
}
