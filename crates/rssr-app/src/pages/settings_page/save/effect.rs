use rssr_domain::UserSettings;

#[derive(Debug, Clone)]
pub(crate) enum SettingsPageSaveEffect {
    SaveAppearance { settings: UserSettings, success_message: String },
}
