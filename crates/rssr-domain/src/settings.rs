use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    Light,
    Dark,
    #[default]
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ListDensity {
    #[default]
    Comfortable,
    Compact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StartupView {
    #[default]
    All,
    LastFeed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UserSettings {
    pub theme: ThemeMode,
    pub list_density: ListDensity,
    pub startup_view: StartupView,
    pub refresh_interval_minutes: u32,
    pub reader_font_scale: f32,
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            theme: ThemeMode::System,
            list_density: ListDensity::Comfortable,
            startup_view: StartupView::All,
            refresh_interval_minutes: 30,
            reader_font_scale: 1.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigFeed {
    pub url: String,
    pub title: Option<String>,
    pub folder: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigPackage {
    pub version: u32,
    pub exported_at: OffsetDateTime,
    pub feeds: Vec<ConfigFeed>,
    pub settings: UserSettings,
}
