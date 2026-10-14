#![cfg(not(target_arch = "wasm32"))]

//! The settings repository against the v0.2 library store (ports of
//! `test_settings_repository.rs`).

#[path = "support/library.rs"]
mod library;

use library::Library;
use rssr_application::{SettingsPatch, SettingsUpdatePort};
use rssr_domain::{ListDensity, SettingsRepository, StartupView, ThemeMode, UserSettings};

#[tokio::test]
async fn settings_repository_persists_and_loads_settings() {
    let library = Library::memory().await;
    let settings = UserSettings {
        theme: ThemeMode::Dark,
        list_density: ListDensity::Compact,
        startup_view: StartupView::LastFeed,
        refresh_interval_minutes: 45,
        archive_after_months: 6,
        entries_page_size: 100,
        reader_font_scale: 1.2,
        custom_css: "[data-page=\"feeds\"] { opacity: 0.95; }".to_string(),
    };

    library.settings.save(&settings).await.expect("save settings");
    let loaded = library.settings.load().await.expect("load settings");

    assert_eq!(loaded, settings);
}

#[tokio::test]
async fn settings_repository_loads_legacy_settings_without_custom_css() {
    let library = Library::memory().await;

    library
        .exec(
            r#"INSERT INTO app_settings (key, value, updated_at) VALUES ('user_settings', '{
            "theme":"light",
            "list_density":"comfortable",
            "startup_view":"all",
            "refresh_interval_minutes":30,
            "reader_font_scale":1.0
        }', '1970-01-01T00:00:00Z')"#,
        )
        .await;

    let loaded = library.settings.load().await.expect("load legacy settings");

    assert_eq!(loaded.custom_css, "");
    assert_eq!(loaded.archive_after_months, 3);
    assert_eq!(loaded.entries_page_size, 100);
    assert_eq!(loaded.theme, ThemeMode::Light);
}

/// A patch is merged in the write: only its fields change, a no-op writes nothing, an invalid
/// patch leaves the stored settings as they were, and stored CSS from before the check does not
/// block an unrelated field.
#[tokio::test]
async fn settings_updates_merge_in_one_write() {
    let library = Library::memory().await;
    let stored = UserSettings { custom_css: "a { color: red;".into(), ..UserSettings::default() };
    library.settings.save(&stored).await.expect("save settings");
    library.exec("UPDATE app_settings SET updated_at = 'before' WHERE key = 'user_settings'").await;
    let updated_at = "SELECT updated_at FROM app_settings WHERE key = 'user_settings'";

    let same = SettingsPatch { theme: Some(stored.theme), ..SettingsPatch::default() };
    assert_eq!(library.settings.update_settings(same).await.unwrap(), stored);
    assert_eq!(library.text(updated_at).await.as_deref(), Some("before"), "a no-op writes nothing");

    let dark = SettingsPatch { theme: Some(ThemeMode::Dark), ..SettingsPatch::default() };
    let saved = library.settings.update_settings(dark).await.expect("unrelated to the CSS");
    assert_eq!(saved, UserSettings { theme: ThemeMode::Dark, ..stored.clone() });
    assert_eq!(library.settings.load().await.unwrap(), saved);

    let invalid = SettingsPatch { entries_page_size: Some(0), ..SettingsPatch::default() };
    let error = library.settings.update_settings(invalid).await.unwrap_err();
    assert!(error.to_string().contains("文章页每页数量"), "{error}");
    let invalid_css = SettingsPatch { custom_css: Some("b {".into()), ..SettingsPatch::default() };
    assert!(library.settings.update_settings(invalid_css).await.is_err());
    assert_eq!(library.settings.load().await.unwrap(), saved);
}
