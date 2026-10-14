#![cfg(not(target_arch = "wasm32"))]

//! The configuration package format does not change (B2-D7): a package exported by the official
//! v0.1.23 CLI (`fixtures/config/v0.1.23-export.json`, generated with `import-opml`,
//! `save-settings` and `export-config`) decodes, and encodes back to the same text.

use rssr_domain::{StartupView, ThemeMode};
use rssr_infra::config_sync::file_format::{decode_config_package, encode_config_package};

const EXPORTED: &str = include_str!("fixtures/config/v0.1.23-export.json");

#[test]
fn a_package_exported_by_v0_1_23_round_trips_unchanged() {
    // A checkout may turn the line endings into CRLF; the format itself writes LF.
    let exported = EXPORTED.replace("\r\n", "\n");
    let package = decode_config_package(&exported).expect("decode the v0.1.23 export");

    assert_eq!(package.version, 2);
    let urls: Vec<_> = package.feeds.iter().map(|feed| feed.url.as_str()).collect();
    assert_eq!(
        urls,
        [
            "https://bare.example.net/atom.xml",
            "https://example.com/feed.xml",
            "https://news.example.org/rss"
        ]
    );
    assert_eq!(package.feeds[1].folder.as_deref(), Some("Tech"));
    assert_eq!(
        (package.settings.theme, package.settings.startup_view),
        (ThemeMode::Dark, StartupView::LastFeed)
    );
    assert_eq!(package.settings.refresh_interval_minutes, 45);

    assert_eq!(encode_config_package(&package).expect("encode"), exported.trim_end());
}
