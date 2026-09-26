use rssr_application::{RefreshHttpMetadata, SubscriptionProbeOutcome};
use rssr_infra::subscription_probe::{MAX_SUBSCRIPTION_BYTES, classify_subscription_response};
use url::Url;

pub fn assert_discovery_cases() {
    let url = Url::parse("https://example.com/moved/page").unwrap();
    let html = r#"<!doctype html><HTML><HEAD><BASE HREF='../rss/'>
        <SCRIPT>var fake = '<link rel="alternate" type="application/rss+xml" href="fake">';</SCRIPT>
        <link REL='alternate stylesheet' TYPE='APPLICATION/RSS+XML' href='news.xml#fragment' title='新闻 &amp; 评论'>
        <link rel=alternate type=application/rss+xml href='news.xml'>
        <link rel=alternate type=application/atom+xml href='/atom.xml'>
        <link rel=alternate type=application/rss+xml href='javascript:bad'>
        </HEAD><BODY><link rel=alternate type=application/rss+xml href='/ignored'></BODY>"#;
    let SubscriptionProbeOutcome::Html { page_url, candidates } = classify_subscription_response(
        url.clone(),
        RefreshHttpMetadata::default(),
        "text/html; charset=utf-8",
        html,
    )
    .unwrap() else {
        panic!("html");
    };
    assert_eq!(page_url, url);
    assert_eq!(candidates.len(), 2);
    assert_eq!(candidates[0].url.as_str(), "https://example.com/rss/news.xml");
    assert_eq!(candidates[0].title.as_deref(), Some("新闻 & 评论"));
    assert_eq!(candidates[1].url.as_str(), "https://example.com/atom.xml");
    let SubscriptionProbeOutcome::Html { candidates, .. } = classify_subscription_response(
        url.clone(),
        Default::default(),
        "text/html",
        "<head><link rel=alternate type=application/atom+xml href='//other.test/feed'>",
    )
    .unwrap() else {
        panic!("malformed html");
    };
    assert_eq!(candidates[0].url.as_str(), "https://other.test/feed");
    let rss = r#"<rss version="2.0"><channel><title>Feed</title><description>test</description><link>https://feed-site.example/</link></channel></rss>"#;
    assert!(matches!(
        classify_subscription_response(url.clone(), Default::default(), "text/html", rss).unwrap(),
        SubscriptionProbeOutcome::Feed { .. }
    ));
    assert!(
        classify_subscription_response(url.clone(), Default::default(), "application/json", "{}")
            .is_err()
    );
    assert!(
        classify_subscription_response(
            url,
            Default::default(),
            "text/html",
            &"x".repeat(MAX_SUBSCRIPTION_BYTES + 1)
        )
        .is_err()
    );
}
