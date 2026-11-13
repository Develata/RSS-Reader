use rssr_application::{RemoteEndpoint, RemoteTarget, SettingsPatch};

#[derive(Debug, Clone)]
pub(crate) enum SettingsCommand {
    Load,
    /// Saves only the fields the page changed; the reply carries every stored field.
    UpdateSettings {
        patch: SettingsPatch,
        success_message: String,
    },
    PushConfig {
        target: RemoteTarget,
    },
    PullConfig {
        target: RemoteTarget,
    },
    /// Reads whether automatic synchronization is available and its saved endpoint.
    LoadRemoteSync,
    /// Keeps the endpoint for automatic synchronization.
    SaveRemoteSync(RemoteEndpoint),
    /// Forgets the saved endpoint and its credentials.
    ClearRemoteSync,
}
