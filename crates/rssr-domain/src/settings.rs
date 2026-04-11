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
    #[serde(default = "default_archive_after_months")]
    pub archive_after_months: u32,
    pub reader_font_scale: f32,
    #[serde(default)]
    pub custom_css: String,
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            theme: ThemeMode::System,
            list_density: ListDensity::Comfortable,
            startup_view: StartupView::All,
            refresh_interval_minutes: 30,
            archive_after_months: default_archive_after_months(),
            reader_font_scale: 1.0,
            custom_css: String::new(),
        }
    }
}

const fn default_archive_after_months() -> u32 {
    3
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigFeed {
    pub url: String,
    pub title: Option<String>,
    // 为了保留 OPML / 配置交换中的原始分组信息，仍然继续序列化该字段。
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
