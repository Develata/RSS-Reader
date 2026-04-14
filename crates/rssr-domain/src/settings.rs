use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

pub const DEFAULT_ENTRIES_PAGE_SIZE: u32 = 100;
pub const MAX_ENTRIES_PAGE_SIZE: u32 = 200;

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
    #[serde(default = "default_entries_page_size")]
    pub entries_page_size: u32,
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
            entries_page_size: default_entries_page_size(),
            reader_font_scale: 1.0,
            custom_css: String::new(),
        }
    }
}

const fn default_archive_after_months() -> u32 {
    3
}

const fn default_entries_page_size() -> u32 {
    DEFAULT_ENTRIES_PAGE_SIZE
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

#[cfg(test)]
mod tests {
    use super::{DEFAULT_ENTRIES_PAGE_SIZE, UserSettings};

    #[test]
    fn user_settings_defaults_entries_page_size_for_legacy_payloads() {
        let settings: UserSettings = serde_json::from_str(
            r#"{
                "theme":"system",
                "list_density":"comfortable",
                "startup_view":"all",
                "refresh_interval_minutes":30,
                "archive_after_months":3,
                "reader_font_scale":1.0,
                "custom_css":""
            }"#,
        )
        .expect("deserialize legacy user settings");

        assert_eq!(settings.entries_page_size, DEFAULT_ENTRIES_PAGE_SIZE);
    }
}
