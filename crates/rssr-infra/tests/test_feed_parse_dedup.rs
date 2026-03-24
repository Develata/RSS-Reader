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
