#![cfg(not(target_arch = "wasm32"))]

use rssr_infra::parser::FeedParser;

const SAMPLE_FEED: &str = r#"
<rss version="2.0">
  <channel>
    <title>Example Feed</title>
    <link>https://example.com</link>
    <description>Example description</description>
    <item>
      <guid>entry-1</guid>
      <title>Hello World</title>
      <link>https://example.com/hello</link>
      <description>First summary</description>
      <pubDate>Tue, 25 Mar 2026 10:00:00 GMT</pubDate>
    </item>
    <item>
      <title>Hello World</title>
      <link>https://example.com/hello</link>
      <description>Updated summary</description>
      <pubDate>Tue, 25 Mar 2026 10:00:00 GMT</pubDate>
    </item>
  </channel>
</rss>
"#;

const HTML_ONLY_FEED: &str = r#"
<rss version="2.0">
  <channel>
    <title>HTML Feed</title>
    <item>
      <guid>entry-html</guid>
      <title>HTML entry</title>
      <link>https://example.com/html</link>
      <content:encoded xmlns:content="http://purl.org/rss/1.0/modules/content/"><![CDATA[<p>Hello <strong>HTML</strong></p>]]></content:encoded>
      <pubDate>Tue, 25 Mar 2026 10:00:00 GMT</pubDate>
    </item>
  </channel>
</rss>
"#;

const SUMMARY_AND_HTML_FEED: &str = r#"
<rss version="2.0">
  <channel>
    <title>Mixed Feed</title>
    <item>
      <guid>entry-mixed</guid>
      <title>Mixed entry</title>
      <link>https://example.com/mixed</link>
      <description>Short teaser</description>
      <content:encoded xmlns:content="http://purl.org/rss/1.0/modules/content/"><![CDATA[<article><p>Full article body</p></article>]]></content:encoded>
      <pubDate>Tue, 25 Mar 2026 10:00:00 GMT</pubDate>
    </item>
  </channel>
</rss>
"#;

const SPARSE_ITEM_FEED: &str = r#"
<rss version="2.0">
  <channel>
    <title>Sparse Feed</title>
    <item>
      <guid>empty-entry</guid>
      <title>Empty entry</title>
      <link>https://example.com/empty</link>
      <pubDate>Tue, 25 Mar 2026 10:00:00 GMT</pubDate>
    </item>
    <item>
      <guid>valid-entry</guid>
      <title>Valid entry</title>
      <link>https://example.com/valid</link>
      <description>Still readable</description>
      <pubDate>Tue, 25 Mar 2026 11:00:00 GMT</pubDate>
    </item>
  </channel>
</rss>
"#;

#[test]
fn parser_normalizes_feed_and_generates_stable_dedup_keys() {
    let parser = FeedParser::new();
    let parsed = parser.parse(SAMPLE_FEED).expect("parse sample feed");

    assert_eq!(parsed.title.as_deref(), Some("Example Feed"));
    assert_eq!(parsed.entries.len(), 2);
    assert_eq!(parsed.entries[0].dedup_key, "entry-1");
    assert_eq!(parsed.entries[1].dedup_key, "https://example.com/hello");
    assert_eq!(parsed.entries[0].title, "Hello World");
}

#[test]
fn parser_keeps_html_only_entries_in_content_html() {
    let parser = FeedParser::new();
    let parsed = parser.parse(HTML_ONLY_FEED).expect("parse html-only feed");

    assert_eq!(parsed.entries.len(), 1);
    assert!(
        parsed.entries[0]
            .content_html
            .as_deref()
            .unwrap_or_default()
            .contains("<strong>HTML</strong>")
    );
    assert!(parsed.entries[0].content_text.is_none());
}

#[test]
fn parser_preserves_full_html_when_summary_also_exists() {
    let parser = FeedParser::new();
    let parsed = parser.parse(SUMMARY_AND_HTML_FEED).expect("parse mixed-content feed");

    assert_eq!(parsed.entries.len(), 1);
    assert_eq!(parsed.entries[0].content_text.as_deref(), Some("Short teaser"));
    assert!(
        parsed.entries[0].content_html.as_deref().unwrap_or_default().contains("Full article body")
    );
}

/// 产品决策（2026-10-09）：无摘要、无正文的条目保留为仅索引条目（此前整条丢弃），
/// 同一 feed 里的其它条目照常解析。
#[test]
fn parser_keeps_sparse_entries_as_index_only_without_failing_entire_feed() {
    let parser = FeedParser::new();
    let parsed = parser.parse(SPARSE_ITEM_FEED).expect("parse sparse feed");

    assert_eq!(parsed.entries.len(), 2);
    let sparse = &parsed.entries[0];
    assert_eq!(sparse.title, "Empty entry");
    assert_eq!(sparse.url.as_ref().map(url::Url::as_str), Some("https://example.com/empty"));
    assert!(sparse.summary.is_none() && sparse.content_html.is_none());
    assert!(sparse.content_text.is_none());
    assert_eq!(parsed.entries[1].title, "Valid entry");
    assert_eq!(parsed.entries[1].content_text.as_deref(), Some("Still readable"));
}

#[test]
fn parser_preserves_source_hex_guid_when_link_changes() {
    const GUID: &str = "0123456789abcdef0123456789abcdef";
    let parse = |link: &str| {
        let raw = format!(
            r#"<rss version="2.0"><channel><title>Hex GUID</title>
            <item><guid>{GUID}</guid><title>Stable identity</title><link>{link}</link>
            <description>Readable</description></item></channel></rss>"#
        );
        FeedParser::new().parse(&raw).expect("parse hex guid feed").entries.remove(0)
    };

    let before = parse("https://example.com/old");
    let after = parse("https://example.com/new");

    assert_eq!(before.external_id, GUID);
    assert_eq!(before.dedup_key, GUID);
    assert_eq!(after.external_id, GUID);
    assert_eq!(after.dedup_key, GUID);
}

#[test]
fn parser_still_uses_url_when_source_guid_is_missing() {
    let raw = r#"
    <rss version="2.0">
      <channel>
        <title>No GUID</title>
        <item>
          <title>URL identity</title>
          <link>https://example.com/no-guid</link>
          <description>Readable</description>
        </item>
      </channel>
    </rss>
    "#;

    let entry = FeedParser::new().parse(raw).expect("parse no-guid feed").entries.remove(0);

    assert_eq!(entry.external_id, "https://example.com/no-guid");
    assert_eq!(entry.dedup_key, "https://example.com/no-guid");
}
