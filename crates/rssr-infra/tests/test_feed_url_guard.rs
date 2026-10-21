//! feed 地址预检的压力契约：会让 `url` 或 feed-rs 超线性的 feed 在解析之前被整体拒绝，
//! 拒绝本身是线性的；正常 feed 不受影响。不对耗时做断言，CI 作业超时兜底。

use rssr_infra::parser::FeedParser;

fn rejected(raw: &str) -> bool {
    FeedParser::new().parse(raw).is_err_and(|error| format!("{error:#}").contains("拒绝解析 feed"))
}

fn rss_with_item_link(link: &str) -> String {
    format!(
        r#"<rss version="2.0"><channel><title>x</title><link>https://e.test/</link><description>x</description><item><guid isPermaLink="false">x</guid><title>x</title><link>{link}</link><description><![CDATA[<img src="x">]]></description></item></channel></rss>"#
    )
}

#[test]
fn dot_segment_links_are_rejected_before_feed_rs() {
    // `url` 在根路径上处理每个 `..` 都要反扫整个 authority；4 MB 的 feed 曾需要数分钟。
    let link = format!("https://{}@e.test/{}", "a".repeat(1 << 20), "../".repeat(1 << 20));
    assert!(rejected(&rss_with_item_link(&link)));
}

#[test]
fn long_xml_base_is_rejected_before_feed_rs() {
    // 每个元素都会克隆一份当前基地址：1 MB 的 xml:base 配 1000 个条目曾需要约 9 s。
    let entries: String = (0..1_000)
        .map(|i| format!(r#"<entry><id>{i}</id><title>x</title><link href="x{i}"/></entry>"#))
        .collect();
    let atom = format!(
        r#"<feed xmlns="http://www.w3.org/2005/Atom" xml:base="https://e.test/{}/"><id>f</id><title>t</title>{entries}</feed>"#,
        "a".repeat(1 << 20)
    );
    assert!(rejected(&atom));
}

#[test]
fn json_feed_urls_are_checked() {
    let url = format!("https://{}@e.test/{}", "a".repeat(1 << 16), "../".repeat(1 << 16));
    let json = format!(
        r#"{{"version":"https://jsonfeed.org/version/1.1","title":"t","items":[{{"id":"1","url":"{url}"}}]}}"#
    );
    assert!(rejected(&json));
}

#[test]
fn ordinary_feeds_still_parse() {
    let feed = FeedParser::new()
        .parse(&rss_with_item_link("https://e.test/posts/../2026/1.html"))
        .expect("ordinary feed parses");
    assert_eq!(feed.entries.len(), 1);
    assert_eq!(
        feed.entries[0].url.as_ref().map(url::Url::as_str),
        Some("https://e.test/2026/1.html")
    );

    let atom = r#"<feed xmlns="http://www.w3.org/2005/Atom" xml:base="https://e.test/blog/"><id>f</id><title>t</title><entry><id>1</id><title>x</title><link href="posts/1.html"/><summary>Wait... what?</summary></entry></feed>"#;
    let feed = FeedParser::new().parse(atom).expect("atom with xml:base parses");
    assert_eq!(
        feed.entries[0].url.as_ref().map(url::Url::as_str),
        Some("https://e.test/blog/posts/1.html")
    );
}
