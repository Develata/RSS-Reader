#![cfg(any(not(target_arch = "wasm32"), feature = "memory-store"))]

//! The byte preflight runs before namespace resolution and retains the v0.2 model checks.
use rssr_infra::parser::FeedParser;

#[test]
fn long_namespace_values_are_refused_before_resolution() {
    let xml = format!(
        r#"<rss version="2.0" xmlns:x="urn:{}"><channel><title>t</title><link>https://e.test/</link><description>x</description><x:unused/></channel></rss>"#,
        "a".repeat(1024)
    );
    assert!(
        FeedParser::new().parse(&xml).is_err(),
        "namespace values must be bounded before resolving names"
    );
}

#[test]
fn leftover_declarations_do_not_redecode_utf8_or_hide_namespaces() {
    for encoding in ["UTF-16", "UTF-16LE", "UTF-16BE", "GBK", "Shift_JIS"] {
        let xml = format!(
            r#"<?xml version="1.0" encoding="{encoding}"?><rss version="2.0"><channel><title>每日数学</title><link>https://e.test/</link><description>已经解码</description><item><guid>1</guid><title>公式</title><description>正文</description></item></channel></rss>"#
        );
        let feed = FeedParser::new().parse(&xml).expect("already-decoded UTF-8");
        assert_eq!(feed.title.as_deref(), Some("每日数学"), "{encoding}");
        assert_eq!(feed.entries[0].title, "公式", "{encoding}");
        let xml =
            xml.replace("<channel>", &format!(r#"<channel xmlns:x="urn:{}">"#, "x".repeat(1024)));
        assert!(
            FeedParser::new().parse(&xml).is_err(),
            "{encoding}: namespace must still be refused"
        );
    }
}
