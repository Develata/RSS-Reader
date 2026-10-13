#![cfg(not(target_arch = "wasm32"))]

//! The settings repository against the v0.2 library store (ports of
//! `test_settings_repository.rs`).

#[path = "support/library.rs"]
mod library;

use library::Library;
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
