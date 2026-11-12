use rssr_application::{RemoteEndpoint, RemoteSyncView, RemoteTarget, SavedRemote};

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct SettingsPageSyncState {
    pub(crate) endpoint: String,
    /// Typed apart from the endpoint, so no character needs encoding; empty: the endpoint's own.
    pub(crate) username: String,
    pub(crate) password: String,
    /// The password field shows its text (the eye toggle); session-only.
    pub(crate) password_visible: bool,
    pub(crate) remote_path: String,
    pub(crate) pending_remote_pull: bool,
    /// Automatic synchronization as last read; `None` before the first read.
    pub(crate) remote_sync: Option<RemoteSyncView>,
    /// Saving or forgetting the endpoint is in flight: one at a time, and no read applies
    /// meanwhile.
    pub(crate) profile_write_pending: bool,
    /// Bumped by every read and write of the profile: only the latest request's answer applies
    /// (independent review, finding 6; re-review N2).
    pub(crate) profile_request: u64,
    /// A follow-up read of a synchronization in progress is scheduled (one at a time).
    pub(crate) following: bool,
}

/// The typed endpoint without its password (re-review R1).
impl std::fmt::Debug for SettingsPageSyncState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SettingsPageSyncState")
            .field("endpoint", &rssr_application::display_endpoint(&self.endpoint))
            .field("username", &self.username)
            .field("password", &(!self.password.is_empty()).then_some("<redacted>"))
            .field("password_visible", &self.password_visible)
            .field("remote_path", &self.remote_path)
            .field("pending_remote_pull", &self.pending_remote_pull)
            .field("remote_sync", &self.remote_sync)
            .field("profile_write_pending", &self.profile_write_pending)
            .field("profile_request", &self.profile_request)
            .field("following", &self.following)
            .finish()
    }
}

impl SettingsPageSyncState {
    pub(crate) fn new() -> Self {
        Self {
            endpoint: String::new(),
            username: String::new(),
            password: String::new(),
            password_visible: false,
            remote_path: "config/rss-reader.json".to_string(),
            pending_remote_pull: false,
            remote_sync: None,
            profile_write_pending: false,
            profile_request: 0,
            following: false,
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

    /// A read may start (no write in flight); the ticket its answer must carry. Earlier reads
    /// in flight become stale: answers can arrive in any order.
    pub(crate) fn begin_profile_read(&mut self) -> Option<u64> {
        if self.profile_write_pending {
            return None;
        }
        self.profile_request += 1;
        Some(self.profile_request)
    }

    /// Whether the read with `ticket` is still the latest request.
    pub(crate) fn is_current_read(&self, ticket: u64) -> bool {
        !self.profile_write_pending && ticket == self.profile_request
    }

    /// A read's answer, applied only while it is the latest request.
    pub(crate) fn accept_profile_read(&mut self, ticket: u64, view: RemoteSyncView) -> bool {
        if !self.is_current_read(ticket) {
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
        self.profile_request += 1;
        true
    }

    /// Whether a follow-up read should be scheduled: a synchronization is in progress and
    /// none is scheduled yet. Marks it scheduled.
    pub(crate) fn begin_following(&mut self) -> bool {
        let syncing = self.remote_sync.as_ref().is_some_and(|view| view.syncing);
        if !syncing || self.following {
            return false;
        }
        self.following = true;
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
    pub(crate) fn target(&self) -> Result<RemoteTarget, String> {
        Ok(match self.saved_remote() {
            Some(saved) => RemoteTarget::Saved(saved),
            None => RemoteTarget::Endpoint(self.typed_endpoint()?),
        })
    }

    /// The endpoint typed on the page, with the user name and password typed apart (they
    /// replace any in the endpoint). Fails when only a password is typed.
    pub(crate) fn typed_endpoint(&self) -> Result<RemoteEndpoint, String> {
        let endpoint = rssr_application::endpoint_with_credentials(
            &self.endpoint,
            &self.username,
            &self.password,
        )?;
        Ok(RemoteEndpoint { endpoint, remote_path: self.remote_path.clone() })
    }

    pub(crate) fn set_username(&mut self, username: String) {
        self.username = username;
        self.pending_remote_pull = false;
    }

    pub(crate) fn set_password(&mut self, password: String) {
        self.password = password;
        self.pending_remote_pull = false;
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
        assert!(matches!(state.target(), Ok(RemoteTarget::Endpoint(_))));

        state.pending_remote_pull = true;
        state.set_remote_sync(view("https://alice@a.example/"));
        let saved = SavedRemote {
            endpoint: "https://alice@a.example/".into(),
            remote_path: "r.json".into(),
        };
        assert_eq!(state.target(), Ok(RemoteTarget::Saved(saved)));
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

        // Two reads answered out of order: only the later request applies.
        let later = state.begin_profile_read().expect("another read");
        assert!(!state.accept_profile_read(read, saved.clone()), "superseded by the later read");
        assert!(state.accept_profile_read(later, cleared.clone()));
        let read = state.begin_profile_read().expect("a read before the write");

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

    /// One follow-up read at a time, and only while a synchronization is in progress.
    #[test]
    fn a_synchronization_in_progress_is_followed_once() {
        let mut state = SettingsPageSyncState::new();
        assert!(!state.begin_following(), "nothing read yet");
        let ticket = state.begin_profile_read().unwrap();
        let syncing =
            RemoteSyncView { available: true, syncing: true, ..RemoteSyncView::default() };
        state.accept_profile_read(ticket, syncing);
        assert!(state.begin_following());
        assert!(!state.begin_following(), "already scheduled");
        state.following = false;
        let ticket = state.begin_profile_read().unwrap();
        state.accept_profile_read(
            ticket,
            RemoteSyncView { available: true, ..RemoteSyncView::default() },
        );
        assert!(!state.begin_following(), "done: no more reads");
    }

    /// The user name and password typed apart go into the endpoint encoded, whatever they hold;
    /// a password alone is refused; `Debug` never shows it.
    #[test]
    fn credentials_typed_apart_reach_the_endpoint() {
        let mut state = SettingsPageSyncState::new();
        state.endpoint = "https://dav.example/dav/网盘/rssr".into();
        assert_eq!(state.typed_endpoint().unwrap().endpoint, "https://dav.example/dav/网盘/rssr");

        state.set_password("p@ss/1?#%41:x".into());
        assert!(state.typed_endpoint().is_err(), "a password needs a user name");

        state.pending_remote_pull = true;
        state.set_username("Develata".into());
        assert!(!state.pending_remote_pull, "other credentials: confirm again");
        let endpoint = state.typed_endpoint().unwrap().endpoint;
        let url = url::Url::parse(&endpoint).unwrap();
        // Every character but letters and digits encoded, `%` included (the decoding that gives
        // the password back is the WebDAV client's, tested there).
        assert_eq!(url.username(), "Develata");
        assert_eq!(url.password(), Some("p%40ss%2F1%3F%23%2541%3Ax"));
        assert_eq!(url.host_str(), Some("dav.example"));

        let shown = format!("{state:?}");
        assert!(!shown.contains("p@ss") && !shown.contains("p%40ss"), "{shown}");

        // One format: credentials written into the endpoint are refused, fields or not.
        state.endpoint = "https://alice:secret@dav.example/dav/".into();
        let error = state.typed_endpoint().unwrap_err();
        assert!(error.contains("下面两栏"), "{error}");
        state.username.clear();
        state.password.clear();
        assert!(state.typed_endpoint().is_err());
    }

    #[test]
    fn remote_pull_confirmation_is_required_once() {
        let mut state = SettingsPageSyncState::new();

        assert!(state.request_remote_pull_confirmation());
        assert!(state.pending_remote_pull);
        assert!(!state.request_remote_pull_confirmation());
    }
}
