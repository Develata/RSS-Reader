use crate::ui::SettingsCommand;

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

    /// Capture the target before the page starts the task, consuming any old confirmation.
    pub(crate) fn prepare_push(&mut self) -> SettingsCommand {
        self.pending_remote_pull = false;
        SettingsCommand::PushConfig {
            endpoint: self.endpoint.clone(),
            remote_path: self.remote_path.clone(),
        }
    }

    /// None asks the page to show confirmation; a command consumes that confirmation.
    /// This is an interaction rule, not a busy gate or a validation of the target.
    pub(crate) fn prepare_pull(&mut self) -> Option<SettingsCommand> {
        if !self.pending_remote_pull {
            self.pending_remote_pull = true;
            return None;
        }

        self.pending_remote_pull = false;
        Some(SettingsCommand::PullConfig {
            endpoint: self.endpoint.clone(),
            remote_path: self.remote_path.clone(),
        })
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
    fn every_pull_requires_and_consumes_its_own_confirmation() {
        let mut state = SettingsPageSyncState::new();

        for _ in 0..3 {
            assert!(state.prepare_pull().is_none());
            assert!(state.pending_remote_pull);
            assert!(matches!(state.prepare_pull(), Some(SettingsCommand::PullConfig { .. })));
            assert!(!state.pending_remote_pull);
        }
    }

    #[test]
    fn editing_either_target_field_requires_confirmation_again_even_for_the_same_value() {
        for change_endpoint in [true, false] {
            for same_value in [true, false] {
                let mut state = SettingsPageSyncState::new();
                assert!(state.prepare_pull().is_none());
                if change_endpoint {
                    let value = if same_value { state.endpoint.clone() } else { "changed".into() };
                    state.set_endpoint(value);
                } else {
                    let value =
                        if same_value { state.remote_path.clone() } else { "changed".into() };
                    state.set_remote_path(value);
                }
                assert!(!state.pending_remote_pull);
                assert!(state.prepare_pull().is_none());
            }
        }
    }

    #[test]
    fn push_is_immediate_and_cancels_any_pending_pull_confirmation() {
        for pending in [false, true] {
            let mut state = SettingsPageSyncState::new();
            if pending {
                assert!(state.prepare_pull().is_none());
            }
            for _ in 0..2 {
                assert!(matches!(state.prepare_push(), SettingsCommand::PushConfig { .. }));
                assert!(!state.pending_remote_pull);
            }
            assert!(state.prepare_pull().is_none());
        }
    }

    #[test]
    fn prepared_commands_keep_exact_target_values_after_later_edits() {
        // URL validation and normalization remain in the existing host/infra adapters.
        for (endpoint, remote_path) in [
            ("", "config/rss-reader.json"),
            (" https://user:pass@dav.example/base/ ", " /配置%20文件.json "),
            ("https://dav.example/base/", ""),
        ] {
            let mut state = SettingsPageSyncState::new();
            state.set_endpoint(endpoint.into());
            state.set_remote_path(remote_path.into());
            let push = state.prepare_push();
            assert!(state.prepare_pull().is_none());
            let pull = state.prepare_pull().expect("confirmed pull");
            state.set_endpoint("different endpoint".into());
            state.set_remote_path("different path".into());

            for command in [push, pull] {
                let (captured_endpoint, captured_path) = match command {
                    SettingsCommand::PushConfig { endpoint, remote_path }
                    | SettingsCommand::PullConfig { endpoint, remote_path } => {
                        (endpoint, remote_path)
                    }
                    _ => panic!("expected config exchange command"),
                };
                assert_eq!(captured_endpoint, endpoint);
                assert_eq!(captured_path, remote_path);
            }
        }
    }
}
