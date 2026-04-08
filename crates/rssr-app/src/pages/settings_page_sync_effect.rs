#[derive(Debug, Clone)]
pub(crate) enum SettingsPageSyncEffect {
    PushConfig { endpoint: String, remote_path: String },
    PullConfig { endpoint: String, remote_path: String },
}
