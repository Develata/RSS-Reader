use rssr_domain::ConfigFeed;
use rssr_infra::opml::OpmlCodec;

#[test]
fn opml_roundtrip_preserves_titles_and_folders() {
    let codec = OpmlCodec::new();
    let feeds = vec![
        ConfigFeed {
            url: "https://example.com/feed.xml".to_string(),
            title: Some("Example".to_string()),
            folder: Some("Tech".to_string()),
        },
        ConfigFeed {
            url: "https://news.example.com/rss".to_string(),
            title: Some("News".to_string()),
            folder: None,
        },
    ];

    let encoded = codec.encode(&feeds).expect("encode opml");
    let mut decoded = codec.decode(&encoded).expect("decode opml");
    let mut expected = feeds;
    decoded.sort_by(|left, right| left.url.cmp(&right.url));
    expected.sort_by(|left, right| left.url.cmp(&right.url));

    assert_eq!(decoded, expected);
}

#[test]
fn opml_import_ignores_outlines_without_xml_url() {
    let codec = OpmlCodec::new();
    let opml = r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <body>
    <outline text="Tech">
      <outline text="Example" title="Example" type="rss" xmlUrl="https://example.com/feed.xml" />
      <outline text="Missing URL" />
    </outline>
    <outline text="Root Feed" title="Root Feed" type="rss" xmlUrl="https://root.example.com/rss" />
  </body>
</opml>"#;

    let decoded = codec.decode(opml).expect("decode opml");

    assert_eq!(decoded.len(), 2);
    assert_eq!(decoded[0].folder.as_deref(), Some("Tech"));
    assert_eq!(decoded[0].title.as_deref(), Some("Example"));
    assert_eq!(decoded[1].folder, None);
    assert_eq!(decoded[1].url, "https://root.example.com/rss");
}
