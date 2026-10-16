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

    // A tag of 100 000 distinct attributes (the tokenizer compares each with the earlier ones):
    // reading stops at it; the link before it is kept (review C3-R4 R4-01).
    let urls = |candidates: Vec<rssr_application::FeedDiscoveryCandidate>| {
        candidates.into_iter().map(|candidate| candidate.url.to_string()).collect::<Vec<_>>()
    };
    let attributes = (0..100_000).map(|index| format!(" a{index}")).collect::<String>();
    let html = format!(
        "<html><head>{}<meta{attributes}>{}</head></html>",
        link("/before.xml"),
        link("/after.xml")
    );
    assert_eq!(urls(probe(&html)), ["https://example.com/before.xml"]);
    // Duplicate attributes make the tokenizer report errors while the tag is still open: they do
    // not count as reading on.
    let attributes = (0..20_000).map(|index| format!(" d{index} d{index}")).collect::<String>();
    let html = format!(
        "<html><head>{}<meta{attributes}>{}</head></html>",
        link("/before.xml"),
        link("/after.xml")
    );
    assert_eq!(urls(probe(&html)), ["https://example.com/before.xml"]);

    // Many tags each under the token bound: their attributes add up, and reading stops
    // (review C3-R5 R5-01).
    let tag = format!("<meta{}>", (0..8_000).map(|index| format!(" a{index}")).collect::<String>());
    let html = format!(
        "<html><head>{}{}{}</head></html>",
        link("/first.xml"),
        tag.repeat(40),
        link("/late.xml")
    );
    assert_eq!(urls(probe(&html)), ["https://example.com/first.xml"]);

    // A byte order mark inside an address is kept wherever the page splits it into chunks; one at
    // the start is dropped (review C3-R5 R5-04).
    let prefix = "<html><head>";
    let padding = " ".repeat(
        4096 - prefix.len() - "<link rel=alternate type=application/rss+xml href='/a".len(),
    );
    let html = format!("{prefix}{padding}{}</head></html>", link("/a\u{feff}b.xml"));
    assert_eq!(urls(probe(&format!("\u{feff}{html}"))), ["https://example.com/a%EF%BB%BFb.xml"]);

    // A comment of 30 KiB is read through; one of 45 KiB stops reading (R5-05).
    for (size, found) in [(30 * 1024, true), (45 * 1024, false)] {
        let comment = format!("<!--{}-->", "x".repeat(size));
        let html = format!("<html><head>{comment}{}</head></html>", link("/after-comment.xml"));
        assert_eq!(!probe(&html).is_empty(), found, "a comment of {size} bytes");
    }

    // Text, scripts and styles are read as they come: a large inline script does not stop it.
    let script = format!("<script>{}</script>", "var x = 1;\n".repeat(20_000));
    let html = format!("<html><head>{script}{}</head></html>", link("/after-script.xml"));
    assert_eq!(urls(probe(&html)), ["https://example.com/after-script.xml"]);

    // A tag under the bound is read; its title is cut to the bound of a name.
    let titled = format!(
        "<link rel=alternate type=application/rss+xml href='/titled.xml' title='{}'>",
        "t".repeat(20 * 1024)
    );
    let candidates = probe(&format!("<html><head>{titled}</head></html>"));
    assert_eq!(candidates.len(), 1);
    assert!(candidates[0].title.as_ref().unwrap().len() <= rssr_application::MAX_NAME_BYTES);

    // The first <base href> applies even when too long: absolute links are still offered,
    // relative ones are not, and a later base is not used (review C3-R4 R4-03).
    let long_base = format!("<base href='/{}/'>", "b".repeat(3000));
    let html = format!(
        "<html><head>{long_base}<base href='/short/'>{}{}</head></html>",
        link("https://feeds.example/absolute.xml"),
        link("relative.xml")
    );
    assert_eq!(urls(probe(&html)), ["https://feeds.example/absolute.xml"]);

    // Repeated links are collected once: they do not crowd out a distinct one (R4-02).
    let repeated = link("/same.xml").repeat(5000);
    let html = format!("<html><head>{repeated}{}</head></html>", link("/other.xml"));
    assert_eq!(
        urls(probe(&html)),
        ["https://example.com/same.xml", "https://example.com/other.xml"]
    );
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
