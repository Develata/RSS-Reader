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

    pub(crate) fn set_endpoint(&mut self, endpoint: String) {
        self.endpoint = endpoint;
        self.pending_remote_pull = false;
    }

    pub(crate) fn set_remote_path(&mut self, remote_path: String) {
        self.remote_path = remote_path;
        self.pending_remote_pull = false;
    }

    pub(crate) fn request_remote_pull_confirmation(&mut self) -> bool {
        if self.pending_remote_pull {
            return false;
        }

        self.pending_remote_pull = true;
        true
    }

    pub(crate) fn clear_pending_remote_pull(&mut self) {
        self.pending_remote_pull = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_change_clears_pending_remote_pull() {
        let mut state = SettingsPageSyncState::new();
        state.pending_remote_pull = true;

        state.set_endpoint("https://dav.example.com/base/".to_string());

        assert_eq!(state.endpoint, "https://dav.example.com/base/");
        assert!(!state.pending_remote_pull);
    }

    #[test]
    fn remote_path_change_clears_pending_remote_pull() {
        let mut state = SettingsPageSyncState::new();
        state.pending_remote_pull = true;

        state.set_remote_path("other/config.json".to_string());

        assert_eq!(state.remote_path, "other/config.json");
        assert!(!state.pending_remote_pull);
    }

    #[test]
    fn remote_pull_confirmation_is_required_once() {
        let mut state = SettingsPageSyncState::new();

        assert!(state.request_remote_pull_confirmation());
        assert!(state.pending_remote_pull);
        assert!(!state.request_remote_pull_confirmation());
    }
}
