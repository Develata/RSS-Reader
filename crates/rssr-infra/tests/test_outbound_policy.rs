#![cfg(not(target_arch = "wasm32"))]

//! The outbound policy on every request path (v0.2 phase C, C1): feed fetching, subscription
//! probing, image localization and WebDAV. Under `PublicOnly`, a refused destination receives no
//! connection at all, whether it is named by a literal address, reached by a redirect from an
//! allowed host, or behind a host name that resolves to it. Each case has a control: the same
//! request with the destination allowed goes through, so the listeners are known to work.
//!
//! The destinations are loopback listeners: `127.0.0.2` stands for an allowed host (it is listed
//! in the policy), `127.0.0.3` for a refused one, and `localhost` for a host name.

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use base64::Engine as _;
use rssr_application::SubscriptionProbePort;
use rssr_infra::config_sync::webdav::WebDavConfigSync;
use rssr_infra::fetch::{BodyAssetLocalizer, FetchClient, FetchRequest};
use rssr_infra::outbound::{OutboundPolicy, PublicOnly};
use rssr_infra::subscription_probe::HttpSubscriptionProbe;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use url::Url;

const FEED: &str = r#"<?xml version="1.0"?><rss version="2.0"><channel><title>T</title><link>https://example.com/</link><item><title>A</title><link>https://example.com/a</link></item></channel></rss>"#;
const PNG: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=";

/// A listener answering every request with one fixed response, counting the connections it
/// accepted.
struct Server {
    address: SocketAddr,
    connections: Arc<AtomicUsize>,
}

impl Server {
    async fn start(ip: &str, response: Vec<u8>) -> Self {
        let listener = TcpListener::bind((ip, 0)).await.expect("bind a loopback listener");
        let address = listener.local_addr().unwrap();
        let connections = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&connections);
        let response = Arc::new(response);
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                counter.fetch_add(1, Ordering::SeqCst);
                let response = Arc::clone(&response);
                tokio::spawn(async move {
                    let mut request = Vec::new();
                    let mut buffer = [0u8; 4096];
                    while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                        match socket.read(&mut buffer).await {
                            Ok(0) | Err(_) => return,
                            Ok(count) => request.extend_from_slice(&buffer[..count]),
                        }
                    }
                    let _ = socket.write_all(&response).await;
                    let _ = socket.shutdown().await;
                });
            }
        });
        Self { address, connections }
    }

    fn connections(&self) -> usize {
        self.connections.load(Ordering::SeqCst)
    }

    fn url(&self, path: &str) -> String {
        format!("http://{}{path}", self.address)
    }

    fn named_url(&self, path: &str) -> String {
        format!("http://localhost:{}{path}", self.address.port())
    }
}

fn response(status: &str, headers: &[(&str, String)], body: &[u8]) -> Vec<u8> {
    let mut head =
        format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n", body.len());
    for (name, value) in headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str("\r\n");
    let mut bytes = head.into_bytes();
    bytes.extend_from_slice(body);
    bytes
}

fn redirect_to(target: &str) -> Vec<u8> {
    response("302 Found", &[("Location", target.to_owned())], b"")
}

/// `127.0.0.2` allowed by network; `localhost` allowed by name only when `named` is set.
fn policy(named: bool) -> OutboundPolicy {
    let hosts = if named { vec!["localhost".to_owned()] } else { Vec::new() };
    OutboundPolicy::PublicOnly(PublicOnly::new(
        hosts,
        vec!["127.0.0.2/32".parse().unwrap()],
        Vec::new(),
    ))
}

/// The three ways to reach a refused destination, each with no connection to it, and the
/// allowed control.
struct Destinations {
    refused: Server,
    redirector: Server,
    named: Server,
}

impl Destinations {
    async fn start(body: Vec<u8>, path: &str) -> Self {
        let refused = Server::start("127.0.0.3", body.clone()).await;
        let redirector = Server::start("127.0.0.2", redirect_to(&refused.url(path))).await;
        let named = Server::start("127.0.0.1", body).await;
        Self { refused, redirector, named }
    }
}

fn feed_response() -> Vec<u8> {
    response("200 OK", &[("Content-Type", "application/rss+xml".to_owned())], FEED.as_bytes())
}

#[tokio::test(flavor = "multi_thread")]
async fn feed_fetching_reaches_only_allowed_destinations() {
    let at = Destinations::start(feed_response(), "/feed.xml").await;
    let request = |url: String| FetchRequest { url, etag: None, last_modified: None };
    let client = FetchClient::with_policy(policy(false)).unwrap();

    assert!(client.fetch(&request(at.refused.url("/feed.xml"))).await.is_err(), "literal");
    assert!(client.fetch(&request(at.redirector.url("/feed.xml"))).await.is_err(), "redirect");
    assert_eq!(at.redirector.connections(), 1, "the allowed redirector was asked");
    assert!(client.fetch(&request(at.named.named_url("/feed.xml"))).await.is_err(), "name");
    assert_eq!((at.refused.connections(), at.named.connections()), (0, 0));

    let allowed = FetchClient::with_policy(policy(true)).unwrap();
    allowed.fetch(&request(at.named.named_url("/feed.xml"))).await.expect("allowed by name");
    assert_eq!(at.named.connections(), 1);
    FetchClient::new().fetch(&request(at.refused.url("/feed.xml"))).await.expect("unrestricted");
    assert_eq!(at.refused.connections(), 1);
}

/// Every form a URL can give a literal address in (as the URL parser reads it), and a user name
/// that looks like a host: none of them reaches the refused listener.
#[tokio::test(flavor = "multi_thread")]
async fn literal_address_forms_never_reach_a_refused_destination() {
    let at = Destinations::start(feed_response(), "/feed.xml").await;
    let port = at.refused.address.port();
    let client = FetchClient::with_policy(policy(false)).unwrap();
    for url in [
        format!("http://2130706435:{port}/feed.xml"),
        format!("http://0x7f.0.0.3:{port}/feed.xml"),
        format!("http://0177.0.0.3:{port}/feed.xml"),
        format!("http://127.3:{port}/feed.xml"),
        format!("http://%31%32%37.0.0.3:{port}/feed.xml"),
        format!("http://example.com@127.0.0.3:{port}/feed.xml"),
        format!("http://[::ffff:127.0.0.3]:{port}/feed.xml"),
        format!("http://localhost.:{}/feed.xml", at.named.address.port()),
    ] {
        let request = FetchRequest { url: url.clone(), etag: None, last_modified: None };
        assert!(client.fetch(&request).await.is_err(), "{url}");
    }
    assert_eq!((at.refused.connections(), at.named.connections()), (0, 0));
}

#[tokio::test(flavor = "multi_thread")]
async fn subscription_probing_reaches_only_allowed_destinations() {
    let at = Destinations::start(feed_response(), "/feed.xml").await;
    let probe = HttpSubscriptionProbe::with_policy(policy(false)).unwrap();
    let url = |raw: String| Url::parse(&raw).unwrap();

    assert!(probe.probe(&url(at.refused.url("/feed.xml"))).await.is_err(), "literal");
    assert!(probe.probe(&url(at.redirector.url("/feed.xml"))).await.is_err(), "redirect");
    assert_eq!(at.redirector.connections(), 1);
    assert!(probe.probe(&url(at.named.named_url("/feed.xml"))).await.is_err(), "name");
    assert_eq!((at.refused.connections(), at.named.connections()), (0, 0));

    let allowed = HttpSubscriptionProbe::with_policy(policy(true)).unwrap();
    allowed.probe(&url(at.named.named_url("/feed.xml"))).await.expect("allowed by name");
    assert_eq!(at.named.connections(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn image_localization_reaches_only_allowed_destinations() {
    let png = base64::engine::general_purpose::STANDARD.decode(PNG).unwrap();
    let image = response("200 OK", &[("Content-Type", "image/png".to_owned())], &png);
    let at = Destinations::start(image, "/a.png").await;
    let localize = |localizer: BodyAssetLocalizer, src: String| async move {
        localizer.localize_html_images(&format!(r#"<p><img src="{src}"></p>"#), None).await.unwrap()
    };
    let restricted = || BodyAssetLocalizer::background(policy(false)).unwrap();

    for src in [at.refused.url("/a.png"), at.redirector.url("/a.png"), at.named.named_url("/a.png")]
    {
        let html = localize(restricted(), src.clone()).await;
        assert!(!html.contains("data:image/png"), "not localized: {src}");
    }
    assert_eq!(at.redirector.connections(), 1);
    assert_eq!((at.refused.connections(), at.named.connections()), (0, 0));

    let allowed = BodyAssetLocalizer::reader(policy(true)).unwrap();
    assert!(localize(allowed, at.named.named_url("/a.png")).await.contains("data:image/png"));
    assert_eq!(at.named.connections(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn webdav_reaches_only_allowed_destinations() {
    let config = response("200 OK", &[("Content-Type", "application/json".to_owned())], b"{}");
    let at = Destinations::start(config, "/dav/config.json").await;
    let sync = |endpoint: String, named: bool| {
        WebDavConfigSync::with_policy(endpoint, "config.json", policy(named))
    };

    assert!(sync(at.refused.url("/dav/"), false).is_err(), "a literal endpoint is refused");
    let redirected = sync(at.redirector.url("/dav/"), false).unwrap();
    assert!(redirected.download_text().await.is_err(), "redirect");
    assert_eq!(at.redirector.connections(), 1);
    let named = sync(at.named.named_url("/dav/"), false).unwrap();
    assert!(named.download_text().await.is_err(), "name");
    assert_eq!((at.refused.connections(), at.named.connections()), (0, 0));

    let allowed = sync(at.named.named_url("/dav/"), true).unwrap();
    assert_eq!(allowed.download_text().await.unwrap().as_deref(), Some("{}"));
    assert_eq!(at.named.connections(), 1);
}
