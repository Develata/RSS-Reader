use rssr_application::{RemoteEndpoint, RemoteSyncView, RemoteTarget, SavedRemote};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SettingsPageSyncState {
    pub(crate) endpoint: String,
    pub(crate) remote_path: String,
    pub(crate) pending_remote_pull: bool,
    /// Automatic synchronization as last read; `None` before the first read.
    pub(crate) remote_sync: Option<RemoteSyncView>,
    /// Saving or forgetting the endpoint is in flight: one at a time, and no read applies
    /// meanwhile.
    pub(crate) profile_write_pending: bool,
    /// Bumped by every write: a read issued before it is stale (independent review, finding 6).
    pub(crate) profile_epoch: u64,
}

impl SettingsPageSyncState {
    pub(crate) fn new() -> Self {
        Self {
            endpoint: String::new(),
            remote_path: "config/rss-reader.json".to_string(),
            pending_remote_pull: false,
            remote_sync: None,
            profile_write_pending: false,
            profile_epoch: 0,
        }
    }

    /// The saved endpoint as the user sees it.
    fn saved_remote(&self) -> Option<SavedRemote> {
        let profile = self.remote_sync.as_ref()?.profile.as_ref()?;
        Some(SavedRemote {
            endpoint: profile.endpoint.clone(),
            remote_path: profile.remote_path.clone(),
        })
    }

    /// A read may start (no write in flight); its answer applies only with this epoch.
    pub(crate) fn begin_profile_read(&self) -> Option<u64> {
        (!self.profile_write_pending).then_some(self.profile_epoch)
    }

    /// A read's answer, applied unless a write started since it was issued.
    pub(crate) fn accept_profile_read(&mut self, epoch: u64, view: RemoteSyncView) -> bool {
        if self.profile_write_pending || epoch != self.profile_epoch {
            return false;
        }
        self.set_remote_sync(view);
        true
    }

    /// A write may start: none is in flight. Reads in flight become stale.
    pub(crate) fn begin_profile_write(&mut self) -> bool {
        if self.profile_write_pending {
            return false;
        }
        self.profile_write_pending = true;
        self.profile_epoch += 1;
        true
    }

    /// The write ended; its view (when it answered one) is the current one.
    pub(crate) fn finish_profile_write(&mut self, view: Option<RemoteSyncView>) {
        self.profile_write_pending = false;
        if let Some(view) = view {
            self.set_remote_sync(view);
        }
    }

    /// Where an upload or a download goes: the saved endpoint the page shows (the backend
    /// refuses when another is saved by now), else the one typed on the page.
    pub(crate) fn target(&self) -> RemoteTarget {
        match self.saved_remote() {
            Some(saved) => RemoteTarget::Saved(saved),
            None => RemoteTarget::Endpoint(self.typed_endpoint()),
        }
    }

    /// The endpoint typed on the page.
    pub(crate) fn typed_endpoint(&self) -> RemoteEndpoint {
        RemoteEndpoint { endpoint: self.endpoint.clone(), remote_path: self.remote_path.clone() }
    }

    /// A new view: another saved endpoint (or none) asks to confirm a download again, since it
    /// may now go elsewhere (independent review, finding 3).
    fn set_remote_sync(&mut self, view: RemoteSyncView) {
        let saved = view
            .profile
            .as_ref()
            .map(|profile| (profile.endpoint.clone(), profile.remote_path.clone()));
        let before = self.saved_remote().map(|saved| (saved.endpoint, saved.remote_path));
        if saved != before {
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

        let view = |endpoint: &str| RemoteSyncView {
            available: true,
            profile: Some(RemoteSyncProfileView {
                endpoint: endpoint.into(),
                remote_path: "r.json".into(),
                has_base: false,
                last_report: None,
            }),
            ..RemoteSyncView::default()
        };
        let mut state = SettingsPageSyncState::new();
        state.endpoint = "https://dav.example/".into();
        assert!(matches!(state.target(), RemoteTarget::Endpoint(_)));

        state.pending_remote_pull = true;
        state.set_remote_sync(view("https://alice@a.example/"));
        let saved = SavedRemote {
            endpoint: "https://alice@a.example/".into(),
            remote_path: "r.json".into(),
        };
        assert_eq!(state.target(), RemoteTarget::Saved(saved));
        assert!(!state.pending_remote_pull, "the download may go elsewhere now");

        // The same endpoint read again keeps the confirmation; another one does not.
        state.pending_remote_pull = true;
        state.set_remote_sync(view("https://alice@a.example/"));
        assert!(state.pending_remote_pull);
        state.set_remote_sync(view("https://alice@b.example/"));
        assert!(!state.pending_remote_pull, "confirmed for A, not for B");
    }

    /// A read answered after a write started is stale: it does not undo what the write showed.
    #[test]
    fn a_read_issued_before_a_write_does_not_apply_after_it() {
        let saved = RemoteSyncView {
            available: true,
            profile: Some(rssr_application::RemoteSyncProfileView {
                endpoint: "https://dav.example/".into(),
                remote_path: "r.json".into(),
                has_base: true,
                last_report: None,
            }),
            ..RemoteSyncView::default()
        };
        let cleared = RemoteSyncView { available: true, ..RemoteSyncView::default() };
        let mut state = SettingsPageSyncState::new();
        let read = state.begin_profile_read().expect("no write in flight");

        assert!(state.begin_profile_write());
        assert!(!state.begin_profile_write(), "one write at a time");
        assert_eq!(state.begin_profile_read(), None, "no read while a write is in flight");
        state.finish_profile_write(Some(cleared.clone()));
        assert!(!state.accept_profile_read(read, saved.clone()), "the stale read is dropped");
        assert_eq!(state.remote_sync, Some(cleared));

        let read = state.begin_profile_read().expect("a new read");
        assert!(state.accept_profile_read(read, saved.clone()));
        assert_eq!(state.remote_sync, Some(saved));
    }

    #[test]
    fn remote_pull_confirmation_is_required_once() {
        let mut state = SettingsPageSyncState::new();

        assert!(state.request_remote_pull_confirmation());
        assert!(state.pending_remote_pull);
        assert!(!state.request_remote_pull_confirmation());
    }
}
