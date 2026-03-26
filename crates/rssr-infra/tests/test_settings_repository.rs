use rssr_domain::{ListDensity, SettingsRepository, StartupView, ThemeMode, UserSettings};
use rssr_infra::db::{
    migrate, settings_repository::SqliteSettingsRepository, sqlite_native::NativeSqliteBackend,
    storage_backend::StorageBackend,
};

#[tokio::test]
async fn settings_repository_persists_and_loads_settings() {
    let backend = NativeSqliteBackend::new("sqlite::memory:");
    let pool = backend.connect().await.expect("connect sqlite memory");
    migrate(&pool).await.expect("run migrations");

    let repository = SqliteSettingsRepository::new(pool.clone());
    let settings = UserSettings {
        theme: ThemeMode::Dark,
        list_density: ListDensity::Compact,
        startup_view: StartupView::LastFeed,
        refresh_interval_minutes: 45,
        reader_font_scale: 1.2,
        custom_css: "[data-page=\"home\"] { opacity: 0.95; }".to_string(),
    };

    repository.save(&settings).await.expect("save settings");
    let loaded = repository.load().await.expect("load settings");

    assert_eq!(loaded, settings);
}

#[tokio::test]
async fn settings_repository_loads_legacy_settings_without_custom_css() {
    let backend = NativeSqliteBackend::new("sqlite::memory:");
    let pool = backend.connect().await.expect("connect sqlite memory");
    migrate(&pool).await.expect("run migrations");

    sqlx::query(
        "INSERT INTO app_settings (key, value, updated_at) VALUES ('user_settings', ?1, '1970-01-01T00:00:00Z')",
    )
    .bind(
        r#"{
            "theme":"light",
            "list_density":"comfortable",
            "startup_view":"all",
            "refresh_interval_minutes":30,
            "reader_font_scale":1.0
        }"#,
    )
    .execute(&pool)
    .await
    .expect("insert legacy settings");

    let repository = SqliteSettingsRepository::new(pool);
    let loaded = repository.load().await.expect("load legacy settings");

    assert_eq!(loaded.custom_css, "");
    assert_eq!(loaded.theme, ThemeMode::Light);
}
