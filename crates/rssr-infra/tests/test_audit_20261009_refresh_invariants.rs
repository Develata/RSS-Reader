#![cfg(not(target_arch = "wasm32"))]
//! Deterministic regressions for fetch and parse defects found by the 2026-10-09 audit (client
//! identity, GBK feeds, Atom links). The storage cases from the same audit run against the v0.2
//! library in `test_library_audit_20261009_refresh_invariants.rs`. See
//! `docs/engineering-audit-2026-10.md`.

#[path = "support/request_capture.rs"]
mod request_capture;

use request_capture::{Response, Server};
use rssr_infra::{
    fetch::{FetchClient, FetchRequest, FetchResult},
    parser::FeedParser,
};
use url::Url;

/// RFC 9110 §10.1.5: a user agent SHOULD send User-Agent unless configured not to. Feed refresh,
/// subscription discovery and WebDAV identify the client the same way (image localization keeps
/// its own browser-compatible User-Agent).
#[tokio::test]
async fn feed_fetch_identifies_the_client() {
    use rssr_application::SubscriptionProbePort;
    use rssr_infra::{
        config_sync::webdav::WebDavConfigSync, subscription_probe::HttpSubscriptionProbe,
    };

    let server = Server::start(|request| match request.path.as_str() {
        "/feed.xml" => Response::new(
            200,
            "Content-Type: application/rss+xml\r\n",
            r#"<rss version="2.0"><channel><title>T</title></channel></rss>"#,
        ),
        _ => Response::new(404, "", []),
    })
    .await;
    let feed_url = server.url.join("feed.xml").unwrap();
    FetchClient::new()
        .fetch(&FetchRequest { url: feed_url.to_string(), etag: None, last_modified: None })
        .await
        .expect("fetch feed");
    HttpSubscriptionProbe::with_feed_client().probe(&feed_url).await.expect("probe feed");
    let remote = WebDavConfigSync::new(server.url.as_str(), "dav/state.json").expect("webdav");
    assert_eq!(remote.download_text().await.expect("webdav download"), None);

    let requests = server.requests();
    let seen = requests
        .iter()
        .map(|request| (request.method.as_str(), request.path.as_str()))
        .collect::<Vec<_>>();
    assert_eq!(seen, [("GET", "/feed.xml"), ("GET", "/feed.xml"), ("GET", "/dav/state.json")]);
    for request in &requests {
        let user_agent = request.header("user-agent").unwrap_or_default();
        assert!(user_agent.starts_with("RSS-Reader/"), "{} {user_agent:?}", request.path);
    }
}

/// Serves a GBK-encoded feed whose XML prolog declares `encoding="GBK"`, then fetches and parses
/// it through the production transport and parser.
async fn fetch_and_parse_gbk_feed(
    content_type_header: &'static str,
) -> rssr_infra::parser::ParsedFeed {
    fetch_and_parse_gbk_xml(r#"<?xml version="1.0" encoding="GBK"?>"#, content_type_header).await
}

async fn fetch_and_parse_gbk_xml(
    prolog: &str,
    content_type_header: &'static str,
) -> rssr_infra::parser::ParsedFeed {
    let xml = format!(
        r#"{prolog}<rss version="2.0"><channel><title>中文标题</title><item><title>条目一</title><description>正文</description><guid>g1</guid></item></channel></rss>"#
    );
    let (bytes, _, _) = encoding_rs::GBK.encode(&xml);
    let body = bytes.into_owned();
    let server =
        Server::start(move |_| Response::new(200, content_type_header, body.clone())).await;
    let FetchResult::Fetched { body, .. } = FetchClient::new()
        .fetch(&FetchRequest {
            url: server.url.join("gbk.xml").unwrap().to_string(),
            etag: None,
            last_modified: None,
        })
        .await
        .expect("fetch feed")
    else {
        panic!("expected a body");
    };
    FeedParser::new().parse(&body).expect("parse feed")
}

/// XML may declare its encoding only in the prolog (no charset parameter in Content-Type).
/// Decoding the bytes as UTF-8 before the XML parser sees them destroys GBK/Big5/Shift_JIS text.
#[tokio::test]
async fn prolog_declared_gbk_feed_keeps_its_text() {
    let parsed = fetch_and_parse_gbk_feed("Content-Type: application/rss+xml\r\n").await;
    assert_eq!(parsed.title.as_deref(), Some("中文标题"));
    assert_eq!(parsed.entries[0].title, "条目一");
}

/// With both an HTTP charset and a matching prolog declaration, the transport transcodes to UTF-8
/// but the parser still honours the stale `encoding="GBK"` declaration and decodes a second time.
#[tokio::test]
async fn http_and_prolog_both_gbk_feed_keeps_its_text() {
    let parsed =
        fetch_and_parse_gbk_feed("Content-Type: application/rss+xml; charset=GBK\r\n").await;
    assert_eq!(parsed.title.as_deref(), Some("中文标题"));
    assert_eq!(parsed.entries[0].title, "条目一");
}

/// XML 1.0 §2.8 puts no bound on the whitespace inside the declaration, so neither the encoding
/// sniff nor pinning the parser to UTF-8 may rely on a fixed lookahead (independent review).
#[tokio::test]
async fn padded_gbk_declaration_is_decoded_once_with_or_without_http_charset() {
    let prolog = format!(r#"<?xml version="1.0" encoding="GBK"{}?>"#, " ".repeat(1100));
    for header in [
        "Content-Type: application/rss+xml; charset=GBK\r\n",
        "Content-Type: application/rss+xml\r\n",
    ] {
        let parsed = fetch_and_parse_gbk_xml(&prolog, header).await;
        assert_eq!(parsed.title.as_deref(), Some("中文标题"), "{header}");
        assert_eq!(parsed.entries[0].title, "条目一", "{header}");
    }
}

/// Atom lists links by relation. The feed's site is `rel="alternate"` (not `rel="self"`), and an
/// entry's article is `rel="alternate"` (not `replies`/`enclosure`), regardless of order.
#[test]
fn atom_links_are_selected_by_relation_not_position() {
    let parsed = FeedParser::new()
        .parse(
            r#"<feed xmlns="http://www.w3.org/2005/Atom"><title>F</title><id>f</id>
 <link rel="self" href="https://example.com/feed.atom"/>
 <link rel="alternate" href="https://example.com/"/>
 <entry><id>e1</id><title>E</title><summary>S</summary>
  <link rel="replies" href="https://example.com/e1/comments"/>
  <link rel="alternate" href="https://example.com/e1"/>
 </entry></feed>"#,
        )
        .expect("parse atom");
    assert_eq!(parsed.site_url.as_ref().map(Url::as_str), Some("https://example.com/"));
    assert_eq!(parsed.entries[0].url.as_ref().map(Url::as_str), Some("https://example.com/e1"));
}
