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

/// 名称就是名称，不与订阅地址比较：用户把名称设成地址也照样保留（独立评审）。
#[test]
fn opml_import_keeps_a_name_even_when_it_equals_the_feed_url() {
    let decoded = OpmlCodec::new()
        .decode(&outlines(
            r#"<outline text="https://a.example/feed" title="Source" xmlUrl="https://a.example/feed"/>"#,
        ))
        .expect("decode opml");

    assert_eq!(decoded[0].title.as_deref(), Some("https://a.example/feed"));
}

/// 导出：`text` 写显示名称，`title` 只在知道源标题时写，不拿显示名称冒充；两个名称都没有时
/// `text` 是空串（OPML 2.0 要求有 `text`），再导入时没有名称。文档带 `<head>`。
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

    assert!(encoded.contains("<head/>"), "{encoded}");
    for outline in [
        r#"<outline text="工作资讯" title="Example News" type="rss" xmlUrl="https://a.example/feed"/>"#,
        r#"<outline text="Chosen" type="rss" xmlUrl="https://b.example/feed"/>"#,
        r#"<outline text="" type="rss" xmlUrl="https://c.example/feed"/>"#,
    ] {
        assert!(encoded.contains(outline), "{outline} in {encoded}");
    }
    let decoded = codec.decode(&encoded).expect("decode opml");
    let names: Vec<_> = decoded.iter().map(|feed| feed.title.as_deref()).collect();
    assert_eq!(names, [Some("工作资讯"), Some("Chosen"), None]);
}

/// 属性里的制表、换行、回车写成字符引用（否则其他 XML 解析器会把它们变成空格），XML 1.0 不
/// 允许的字符去掉，导出的文件始终是合法 XML。只看输出的原文：本 codec 自己读回时不做属性值
/// 规范化，往返测不出这个问题（独立评审）。
#[test]
fn opml_export_escapes_whitespace_and_drops_characters_xml_forbids() {
    let encoded = OpmlCodec::new()
        .encode(&[OpmlFeed {
            url: "https://a.example/feed?a=1&b=2".to_string(),
            name: Some("Line\none\ttab\r\u{1}\"q\" <x> 'y'".to_string()),
            source_title: Some("Source\u{FFFE}\u{0}".to_string()),
            folder: Some("Folder\nTwo".to_string()),
        }])
        .expect("encode opml");

    assert!(
        encoded.contains(
            r#"text="Line&#10;one&#9;tab&#13;&quot;q&quot; &lt;x&gt; &apos;y&apos;" title="Source""#
        ),
        "{encoded}"
    );
    assert!(encoded.contains(r#"xmlUrl="https://a.example/feed?a=1&amp;b=2""#), "{encoded}");
    assert!(
        encoded.contains(r#"<outline text="Folder&#10;Two" title="Folder&#10;Two">"#),
        "{encoded}"
    );
    assert!(!encoded.contains(['\u{0}', '\u{1}', '\u{FFFE}', '\t', '\r']), "{encoded:?}");
    let decoded = OpmlCodec::new().decode(&encoded).expect("decode opml");
    assert_eq!(decoded[0].title.as_deref(), Some("Line\none\ttab\r\"q\" <x> 'y'"));
    assert_eq!(decoded[0].folder.as_deref(), Some("Folder\nTwo"));
}

/// 订阅出现在 <body> 外、或任何元素的属性写坏时整体拒绝，不导入其中一部分（独立评审：原先
/// <body> 外的订阅被静默忽略，却报告导入成功）。
#[test]
fn opml_import_rejects_outlines_outside_the_body_and_broken_attributes() {
    let codec = OpmlCodec::new();
    for raw in [
        r#"<opml version="2.0"><head/><body><outline text="A" xmlUrl="https://a.example/feed"/></body><outline text="B" xmlUrl="https://b.example/feed"/></opml>"#,
        r#"<opml version="2.0"><head><outline text="B" xmlUrl="https://b.example/feed"/></head><body><outline text="A" xmlUrl="https://a.example/feed"/></body></opml>"#,
        r#"<opml version="2.0"><body bad=bare><outline text="A" xmlUrl="https://a.example/feed"/></body></opml>"#,
    ] {
        assert!(codec.decode(raw).is_err(), "{raw}");
    }
}
