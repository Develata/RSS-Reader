use rssr_domain::{ConfigFeed, OpmlFeed};
use rssr_infra::opml::OpmlCodec;

#[test]
fn opml_roundtrip_preserves_titles_and_folders() {
    let codec = OpmlCodec::new();
    let feeds = vec![
        OpmlFeed {
            url: "https://example.com/feed.xml".to_string(),
            name: Some("Example".to_string()),
            source_title: Some("Example Source".to_string()),
            folder: Some("Tech".to_string()),
        },
        OpmlFeed {
            url: "https://news.example.com/rss".to_string(),
            name: Some("News".to_string()),
            source_title: None,
            folder: None,
        },
    ];

    let encoded = codec.encode(&feeds).expect("encode opml");
    let mut decoded = codec.decode(&encoded).expect("decode opml");
    let mut expected: Vec<ConfigFeed> = feeds
        .into_iter()
        .map(|feed| ConfigFeed { url: feed.url, title: feed.name, folder: feed.folder })
        .collect();
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

fn outlines(body: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><opml version="2.0"><body>{body}</body></opml>"#
    )
}

/// OPML 2.0：`text` 是显示的名称（其他阅读器里改过的名称在这里），`title` 来自订阅源。导入
/// 取 `text`，空白时退回 `title`；分组也一样（审计 B6）。
#[test]
fn opml_import_prefers_text_and_falls_back_to_title() {
    let decoded = OpmlCodec::new()
        .decode(&outlines(
            r#"<outline text="工作" title="Work"><outline text="工作资讯" title="Example News" xmlUrl="https://a.example/feed"/>
               <outline text="  " title="Example B" xmlUrl="https://b.example/feed"/>
               <outline title="Example C" xmlUrl="https://c.example/feed"/></outline>
               <outline xmlUrl="https://d.example/feed"/>"#,
        ))
        .expect("decode opml");

    let names: Vec<_> = decoded
        .iter()
        .map(|feed| (feed.url.as_str(), feed.title.as_deref(), feed.folder.as_deref()))
        .collect();
    assert_eq!(
        names,
        [
            ("https://a.example/feed", Some("工作资讯"), Some("工作")),
            ("https://b.example/feed", Some("Example B"), Some("工作")),
            ("https://c.example/feed", Some("Example C"), Some("工作")),
            ("https://d.example/feed", None, None),
        ]
    );
}

/// 没有名称时本应用和其他阅读器都把订阅地址写进 `text`；与 `xmlUrl` 相同的名称不算名称，
/// 否则再导入会把地址固定成指定名称。
#[test]
fn opml_import_ignores_a_name_that_is_just_the_feed_url() {
    let decoded = OpmlCodec::new()
        .decode(&outlines(
            r#"<outline text="https://a.example/feed" title="https://a.example/feed" xmlUrl="https://a.example/feed"/>
               <outline text=" https://b.example/feed " title="Example B" xmlUrl="https://b.example/feed"/>"#,
        ))
        .expect("decode opml");

    assert_eq!(decoded[0].title, None);
    assert_eq!(decoded[1].title, None, "text equal to the URL is not a name; no fallback to title");
}

/// 导出：`text` 写显示名称，`title` 只在知道源标题时写，不拿显示名称冒充；两个名称都没有时
/// `text` 写订阅地址（OPML 2.0 要求 `text`），再导入时不会变成名称。
#[test]
fn opml_export_writes_the_display_name_and_only_a_known_source_title() {
    let codec = OpmlCodec::new();
    let encoded = codec
        .encode(&[
            OpmlFeed {
                url: "https://a.example/feed".to_string(),
                name: Some("工作资讯".to_string()),
                source_title: Some("Example News".to_string()),
                folder: None,
            },
            OpmlFeed {
                url: "https://b.example/feed".to_string(),
                name: Some("Chosen".to_string()),
                source_title: None,
                folder: None,
            },
            OpmlFeed {
                url: "https://c.example/feed".to_string(),
                name: None,
                source_title: None,
                folder: None,
            },
        ])
        .expect("encode opml");

    assert!(
        encoded.contains(r#"<outline text="工作资讯" title="Example News" type="rss" xmlUrl="https://a.example/feed"/>"#),
        "{encoded}"
    );
    assert!(
        encoded.contains(r#"<outline text="Chosen" type="rss" xmlUrl="https://b.example/feed"/>"#),
        "{encoded}"
    );
    assert!(
        encoded.contains(
            r#"<outline text="https://c.example/feed" type="rss" xmlUrl="https://c.example/feed"/>"#
        ),
        "{encoded}"
    );
    let decoded = codec.decode(&encoded).expect("decode opml");
    let names: Vec<_> = decoded.iter().map(|feed| feed.title.as_deref()).collect();
    assert_eq!(names, [Some("工作资讯"), Some("Chosen"), None]);
}
