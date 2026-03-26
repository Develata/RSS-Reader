use rssr_domain::{ConfigFeed, ConfigPackage, ListDensity, StartupView, ThemeMode, UserSettings};
use rssr_infra::config_sync::file_format::{decode_config_package, encode_config_package};
use serde_json::Value;
use time::OffsetDateTime;

fn schema() -> Value {
    serde_json::from_str(include_str!(
        "../../../specs/001-minimal-rss-reader/contracts/config-package.schema.json"
    ))
    .expect("schema should be valid json")
}

#[test]
fn config_package_schema_matches_runtime_contract() {
    let schema = schema();

    assert_eq!(schema["type"], "object");
    assert_eq!(schema["additionalProperties"], false);
    assert_eq!(
        schema["required"],
        serde_json::json!(["version", "exported_at", "feeds", "settings"])
    );

    let feed_items = &schema["properties"]["feeds"]["items"];
    assert_eq!(feed_items["required"], serde_json::json!(["url"]));
    assert_eq!(feed_items["additionalProperties"], false);
    assert_eq!(feed_items["properties"]["url"]["format"], "uri");
    assert_eq!(feed_items["properties"]["title"]["type"], serde_json::json!(["string", "null"]));
    assert_eq!(feed_items["properties"]["folder"]["type"], serde_json::json!(["string", "null"]));

    let settings = &schema["properties"]["settings"];
    assert_eq!(settings["additionalProperties"], false);
    assert_eq!(
        settings["required"],
        serde_json::json!([
            "theme",
            "list_density",
            "startup_view",
            "refresh_interval_minutes",
            "reader_font_scale",
            "custom_css"
        ])
    );
    assert_eq!(
        settings["properties"]["theme"]["enum"],
        serde_json::json!(["light", "dark", "system"])
    );
    assert_eq!(
        settings["properties"]["list_density"]["enum"],
        serde_json::json!(["comfortable", "compact"])
    );
    assert_eq!(
        settings["properties"]["startup_view"]["enum"],
        serde_json::json!(["all", "last_feed"])
    );
    assert_eq!(settings["properties"]["refresh_interval_minutes"]["minimum"], 1);
    assert_eq!(settings["properties"]["reader_font_scale"]["minimum"], 0.8);
    assert_eq!(settings["properties"]["reader_font_scale"]["maximum"], 1.5);
    assert_eq!(settings["properties"]["custom_css"]["type"], "string");
}

#[test]
fn encoded_config_package_uses_schema_field_names_and_enum_values() {
    let package = ConfigPackage {
        version: 1,
        exported_at: OffsetDateTime::UNIX_EPOCH,
        feeds: vec![ConfigFeed {
            url: "https://example.com/feed.xml".to_string(),
            title: Some("Example".to_string()),
            folder: Some("Tech".to_string()),
        }],
        settings: UserSettings {
            theme: ThemeMode::Dark,
            list_density: ListDensity::Compact,
            startup_view: StartupView::LastFeed,
            refresh_interval_minutes: 15,
            reader_font_scale: 1.2,
            custom_css: "[data-page=\"reader\"] .reader-body { max-width: 70ch; }".to_string(),
        },
    };

    let encoded = encode_config_package(&package).expect("config package should encode");
    let json: Value = serde_json::from_str(&encoded).expect("encoded config package is valid json");

    assert_eq!(json["version"], 1);
    assert!(json.get("exported_at").and_then(Value::as_str).is_some());
    assert_eq!(json["feeds"][0]["url"], "https://example.com/feed.xml");
    assert_eq!(json["feeds"][0]["title"], "Example");
    assert_eq!(json["feeds"][0]["folder"], "Tech");
    assert_eq!(json["settings"]["theme"], "dark");
    assert_eq!(json["settings"]["list_density"], "compact");
    assert_eq!(json["settings"]["startup_view"], "last_feed");
    assert_eq!(json["settings"]["refresh_interval_minutes"], 15);
    assert_eq!(json["settings"]["reader_font_scale"], 1.2);
    assert_eq!(
        json["settings"]["custom_css"],
        "[data-page=\"reader\"] .reader-body { max-width: 70ch; }"
    );

    let decoded = decode_config_package(&encoded).expect("encoded package should decode");
    assert_eq!(decoded, package);
}
