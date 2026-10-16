use rssr_application::{RefreshHttpMetadata, SubscriptionProbeOutcome};
use rssr_infra::subscription_probe::{MAX_SUBSCRIPTION_BYTES, classify_subscription_response};
use url::Url;

/// Discovery is bounded while the page is read (review C3-R3 R3-01): a long `<base>` gives no
/// candidates, long links are left out, and at most `MAX_DISCOVERY_CANDIDATES` distinct feeds are
/// kept.
pub fn assert_discovery_bounds() {
    use rssr_application::{MAX_DISCOVERY_CANDIDATES, MAX_URL_BYTES};
    let url = Url::parse("https://example.com/page").unwrap();
    let probe = |html: &str| match classify_subscription_response(
        url.clone(),
        RefreshHttpMetadata::default(),
        "text/html",
        html,
    )
    .unwrap()
    {
        SubscriptionProbeOutcome::Html { candidates, .. } => candidates,
        _ => panic!("an HTML page"),
    };
    let link = |href: &str| format!("<link rel=alternate type=application/rss+xml href='{href}'>");

    let long_base = format!("/{}/", "x".repeat(1024 * 1024));
    let links = (0..2500).map(|index| link(&format!("feed{index}.xml"))).collect::<String>();
    let html = format!("<html><head><base href='{long_base}'>{links}</head></html>");
    assert!(probe(&html).is_empty(), "a long base resolves nothing");

    let long_link = link(&format!("/{}", "y".repeat(MAX_URL_BYTES)));
    let many = (0..80).map(|index| link(&format!("/feed{index}.xml"))).collect::<String>();
    let html = format!("<html><head>{long_link}{many}</head></html>");
    let candidates = probe(&html);
    assert_eq!(candidates.len(), MAX_DISCOVERY_CANDIDATES);
    assert!(candidates[0].url.as_str().ends_with("/feed0.xml"), "the long link is left out");
    assert!(candidates.iter().all(|candidate| candidate.url.as_str().len() <= MAX_URL_BYTES));
}

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
