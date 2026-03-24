use rssr_domain::{ConfigFeed, ConfigPackage, ListDensity, StartupView, ThemeMode, UserSettings};
use rssr_infra::config_sync::file_format::{decode_config_package, encode_config_package};
use time::OffsetDateTime;

#[test]
fn config_package_codec_roundtrip() {
    let package = ConfigPackage {
        version: 1,
        exported_at: OffsetDateTime::UNIX_EPOCH,
        feeds: vec![ConfigFeed {
            url: "https://example.com/feed.xml".to_string(),
            title: Some("Example".to_string()),
            folder: Some("Tech".to_string()),
        }],
        settings: UserSettings {
            theme: ThemeMode::System,
            list_density: ListDensity::Compact,
            startup_view: StartupView::All,
            refresh_interval_minutes: 15,
            reader_font_scale: 1.1,
        },
    };

    let encoded = encode_config_package(&package).expect("encode config package");
    let decoded = decode_config_package(&encoded).expect("decode config package");

    assert_eq!(decoded, package);
}
