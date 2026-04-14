#![cfg(not(target_arch = "wasm32"))]

use rssr_domain::{ConfigFeed, ConfigPackage, UserSettings};
use rssr_infra::{
    config_sync::{
        file_format::{decode_config_package, encode_config_package},
        webdav::WebDavConfigSync,
    },
    opml::OpmlCodec,
    parser::FeedParser,
};
use time::OffsetDateTime;

const MIXED_FEED: &str = r#"
<rss version="2.0">
  <channel>
    <title>Regression Feed</title>
    <item>
      <guid>entry-1</guid>
      <title>Readable entry</title>
      <link>https://example.com/entry-1</link>
      <description>Summary text</description>
      <content:encoded xmlns:content="http://purl.org/rss/1.0/modules/content/"><![CDATA[<article><p>Full body</p></article>]]></content:encoded>
      <pubDate>Tue, 25 Mar 2026 10:00:00 GMT</pubDate>
    </item>
    <item>
      <guid>entry-2</guid>
      <title>Sparse entry</title>
      <link>https://example.com/entry-2</link>
      <pubDate>Tue, 25 Mar 2026 11:00:00 GMT</pubDate>
    </item>
  </channel>
</rss>
"#;

#[test]
fn smoke_regressions_cover_parser_config_and_webdav_edges() {
    let parsed = FeedParser::new().parse(MIXED_FEED).expect("parse mixed feed");
    assert_eq!(parsed.entries.len(), 1);
    assert!(
        parsed.entries[0]
            .content_html
            .as_deref()
            .expect("html body preserved")
            .contains("Full body")
    );

    let opml = OpmlCodec::new()
        .encode(&[ConfigFeed {
            url: "https://example.com/feed.xml".to_string(),
            title: Some("Example".to_string()),
            folder: Some("Tech".to_string()),
        }])
        .expect("encode opml");
    let decoded_opml = OpmlCodec::new().decode(&opml).expect("decode opml");
    assert_eq!(decoded_opml.len(), 1);
    assert_eq!(decoded_opml[0].folder.as_deref(), Some("Tech"));

    let raw = encode_config_package(&ConfigPackage {
        version: 2,
        exported_at: OffsetDateTime::UNIX_EPOCH,
        feeds: vec![ConfigFeed {
            url: "https://example.com/feed.xml".to_string(),
            title: Some("Example".to_string()),
            folder: Some("Tech".to_string()),
        }],
        settings: UserSettings::default(),
    })
    .expect("encode config package");
    let config = decode_config_package(&raw).expect("decode config package");
    assert_eq!(config.feeds.len(), 1);

    let remote = WebDavConfigSync::new("https://dav.example.com/base", "config/state.json")
        .expect("create webdav client");
    assert_eq!(
        remote.remote_url().expect("resolve remote url").as_str(),
        "https://dav.example.com/base/config/state.json"
    );
}
