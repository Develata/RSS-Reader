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

#[test]
fn parser_skips_sparse_entries_without_failing_entire_feed() {
    let parser = FeedParser::new();
    let parsed = parser.parse(SPARSE_ITEM_FEED).expect("parse sparse feed");

    assert_eq!(parsed.entries.len(), 1);
    assert_eq!(parsed.entries[0].title, "Valid entry");
}
