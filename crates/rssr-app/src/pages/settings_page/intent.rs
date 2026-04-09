use rssr_domain::UserSettings;

#[derive(Debug, Clone)]
pub(crate) enum SettingsPageIntent {
    SettingsLoaded(UserSettings),
    SetStatus { message: String, tone: String },
}
