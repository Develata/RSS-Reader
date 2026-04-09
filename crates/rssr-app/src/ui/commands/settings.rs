use rssr_domain::UserSettings;

#[derive(Debug, Clone)]
pub(crate) enum SettingsCommand {
    Load,
    SaveAppearance { settings: UserSettings, success_message: String },
    PushConfig { endpoint: String, remote_path: String },
    PullConfig { endpoint: String, remote_path: String },
}
