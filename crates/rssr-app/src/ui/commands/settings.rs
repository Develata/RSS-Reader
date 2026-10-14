use rssr_application::SettingsPatch;

#[derive(Debug, Clone)]
pub(crate) enum SettingsCommand {
    Load,
    /// Saves only the fields the page changed; the reply carries every stored field.
    UpdateSettings {
        patch: SettingsPatch,
        success_message: String,
    },
    PushConfig {
        endpoint: String,
        remote_path: String,
    },
    PullConfig {
        endpoint: String,
        remote_path: String,
    },
}
