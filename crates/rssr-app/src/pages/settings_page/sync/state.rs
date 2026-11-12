use rssr_application::{RemoteSyncView, RemoteTarget};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SettingsPageSyncState {
    pub(crate) endpoint: String,
    pub(crate) remote_path: String,
    pub(crate) pending_remote_pull: bool,
    /// Automatic synchronization as last read; `None` before the first read.
    pub(crate) remote_sync: Option<RemoteSyncView>,
}

impl SettingsPageSyncState {
    pub(crate) fn new() -> Self {
        Self {
            endpoint: String::new(),
            remote_path: "config/rss-reader.json".to_string(),
            pending_remote_pull: false,
            remote_sync: None,
        }
    }

    /// Whether an endpoint is saved for automatic synchronization.
    pub(crate) fn has_saved_endpoint(&self) -> bool {
        self.remote_sync.as_ref().is_some_and(|view| view.profile.is_some())
    }

    /// Where an upload or a download goes: the saved endpoint when there is one, else the one
    /// typed on the page.
    pub(crate) fn target(&self) -> RemoteTarget {
        if self.has_saved_endpoint() {
            RemoteTarget::Saved
        } else {
            RemoteTarget::Endpoint {
                endpoint: self.endpoint.clone(),
                remote_path: self.remote_path.clone(),
            }
        }
    }

    /// A new view: saving or forgetting the endpoint asks to confirm a download again, since
    /// it may now go elsewhere.
    pub(crate) fn set_remote_sync(&mut self, view: RemoteSyncView) {
        if self.has_saved_endpoint() != view.profile.is_some() {
            self.pending_remote_pull = false;
        }
        self.remote_sync = Some(view);
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
    fn a_saved_endpoint_is_the_target_and_changing_it_asks_again() {
        use rssr_application::RemoteSyncProfileView;

        let mut state = SettingsPageSyncState::new();
        state.endpoint = "https://dav.example/".into();
        assert!(matches!(state.target(), RemoteTarget::Endpoint { .. }));

        state.pending_remote_pull = true;
        state.set_remote_sync(RemoteSyncView {
            available: true,
            profile: Some(RemoteSyncProfileView {
                endpoint: "https://alice@dav.example/".into(),
                remote_path: "r.json".into(),
                has_base: false,
                last_report: None,
            }),
        });
        assert_eq!(state.target(), RemoteTarget::Saved);
        assert!(!state.pending_remote_pull, "the download may go elsewhere now");
    }

    #[test]
    fn remote_pull_confirmation_is_required_once() {
        let mut state = SettingsPageSyncState::new();

        assert!(state.request_remote_pull_confirmation());
        assert!(state.pending_remote_pull);
        assert!(!state.request_remote_pull_confirmation());
    }
}
