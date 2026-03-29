use rssr_domain::{ConfigFeed, ConfigPackage, ListDensity, StartupView, ThemeMode, UserSettings};
use rssr_infra::config_sync::file_format::{decode_config_package, encode_config_package};
use serde_json::json;
use time::OffsetDateTime;

#[test]
fn config_package_codec_roundtrip() {
    let package = sample_package();

    let encoded = encode_config_package(&package).expect("encode config package");
    let decoded = decode_config_package(&encoded).expect("decode config package");

    assert_eq!(decoded, package);
}

#[test]
fn config_package_decode_rejects_duplicate_feed_urls() {
    let mut raw = serde_json::to_value(sample_package()).expect("serialize sample package");
    raw["feeds"] = json!([
        { "url": "https://example.com/feed.xml", "title": "A", "folder": null },
        { "url": "https://example.com/feed.xml", "title": "B", "folder": null }
    ]);
    let raw = serde_json::to_string(&raw).expect("encode invalid package");

    let error = decode_config_package(&raw).expect_err("duplicate urls must fail");
    assert!(error.to_string().contains("重复的 feed URL"), "unexpected error: {error:#}");
}

#[test]
fn config_package_decode_rejects_duplicate_feed_urls_after_normalization() {
    let mut raw = serde_json::to_value(sample_package()).expect("serialize sample package");
    raw["feeds"] = json!([
        { "url": "https://example.com/feed.xml#fragment", "title": "A", "folder": null },
        { "url": "https://example.com:443/feed.xml", "title": "B", "folder": null }
    ]);
    let raw = serde_json::to_string(&raw).expect("encode invalid package");

    let error = decode_config_package(&raw).expect_err("normalized duplicate urls must fail");
    assert!(error.to_string().contains("重复的 feed URL"), "unexpected error: {error:#}");
}

#[test]
fn config_package_decode_rejects_invalid_setting_ranges() {
    let mut raw = serde_json::to_value(sample_package()).expect("serialize sample package");
    raw["settings"]["refresh_interval_minutes"] = json!(0);
    raw["settings"]["reader_font_scale"] = json!(2.0);
    let raw = serde_json::to_string(&raw).expect("encode invalid package");

    let error = decode_config_package(&raw).expect_err("invalid settings must fail");
    assert!(error.to_string().contains("刷新间隔必须"), "unexpected error: {error:#}");
}

#[test]
fn config_package_decode_rejects_invalid_feed_url() {
    let mut raw = serde_json::to_value(sample_package()).expect("serialize sample package");
    raw["feeds"][0]["url"] = json!("not-a-url");
    let raw = serde_json::to_string(&raw).expect("encode invalid package");

    let error = decode_config_package(&raw).expect_err("invalid feed url must fail");
    assert!(error.to_string().contains("无效的 feed URL"), "unexpected error: {error:#}");
}

#[test]
fn config_package_decode_rejects_unknown_top_level_property() {
    let mut raw = serde_json::to_value(sample_package()).expect("serialize sample package");
    raw["unexpected"] = json!(true);
    let raw = serde_json::to_string(&raw).expect("encode invalid package");

    let error = decode_config_package(&raw).expect_err("unknown top-level field must fail");
    assert!(error.to_string().contains("unknown field"), "unexpected error: {error:#}");
}

#[test]
fn config_package_decode_rejects_unknown_nested_property() {
    let mut raw = serde_json::to_value(sample_package()).expect("serialize sample package");
    raw["settings"]["unexpected"] = json!(true);
    let raw = serde_json::to_string(&raw).expect("encode invalid package");

    let error = decode_config_package(&raw).expect_err("unknown nested field must fail");
    assert!(error.to_string().contains("unknown field"), "unexpected error: {error:#}");
}

fn sample_package() -> ConfigPackage {
    ConfigPackage {
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
            custom_css: ".feed-card { border-radius: 12px; }".to_string(),
        },
    }
}
