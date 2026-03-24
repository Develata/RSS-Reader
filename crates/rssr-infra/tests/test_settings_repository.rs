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
    };

    repository.save(&settings).await.expect("save settings");
    let loaded = repository.load().await.expect("load settings");

    assert_eq!(loaded, settings);
}
