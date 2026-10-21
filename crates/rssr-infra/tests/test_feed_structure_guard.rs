//! feed 解析的压力契约：点段地址在打过补丁的 `url` 里线性解析，照常放行；会让 feed-rs 或
//! quick-xml 随结构放大的 feed 在解析之前被整体拒绝，拒绝本身是线性的。不对耗时做断言，
//! CI 作业超时兜底。

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
fn dot_segment_links_parse_in_linear_time() {
    // 原版 `url` 2.5.8 在根路径上处理每个 `..` 都要反扫整个 authority，这个 4 MB 的 feed
    // 曾需要数分钟；vendor/url 的补丁之后线性解析，地址照常归一化。
    let n = 1 << 20;
    let link = format!("https://{}@e.test/{}", "a".repeat(n), "../".repeat(n));
    let feed = FeedParser::new().parse(&rss_with_item_link(&link)).expect("feed parses");
    // 超长地址不进入条目（本项目自己的入口有 8 KiB 上限），但解析已经结束。
    assert_eq!(feed.entries.len(), 1);
    assert!(feed.entries[0].url.is_none());
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
fn namespace_and_attribute_abuse_is_rejected_before_feed_rs() {
    // 每层一个命名空间声明的深层嵌套：quick-xml 对每个元素都反向扫描全部绑定。
    let nested = format!(
        r#"<feed xmlns="http://www.w3.org/2005/Atom">{}{}</feed>"#,
        r#"<x xmlns:a="urn:a">"#.repeat(32_000),
        "</x>".repeat(32_000)
    );
    assert!(rejected(&nested));
    // 一个很长的命名空间值配大量带前缀的元素：feed-rs 每次都重新解码它。
    let long_namespace = format!(
        r#"<rss version="2.0"><channel xmlns:x="urn:{}">{}</channel></rss>"#,
        "a".repeat(1 << 20),
        "<x:a/>".repeat(100_000)
    );
    assert!(rejected(&long_namespace));
    // 大量重复属性：quick-xml 对每个重复都线性回查。
    let attributes: String = (0..32_000).map(|i| format!(r#" a{i:05}="""#)).collect();
    let duplicates = format!(r#"<feed{attributes}{}></feed>"#, r#" a31999="""#.repeat(32_000));
    assert!(rejected(&duplicates));
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

    let json = r#"{"version":"https://jsonfeed.org/version/1.1","title":"t","items":[{"id":"1","url":"https://e.test/1","content_text":"x"}]}"#;
    let feed = FeedParser::new().parse(json).expect("json feed parses");
    assert_eq!(feed.entries[0].url.as_ref().map(url::Url::as_str), Some("https://e.test/1"));
}
