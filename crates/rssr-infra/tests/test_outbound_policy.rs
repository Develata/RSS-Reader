#![cfg(not(target_arch = "wasm32"))]

//! The outbound policy on every request path (v0.2 phase C, C1): feed fetching, subscription
//! probing, image localization (background and reader) and WebDAV (download and upload). Under
//! `PublicOnly` a refused destination receives no connection, whether it is named by a literal
//! address, reached by a redirect from an allowed host, or behind a host name whose answers are all
//! refused or mixed (in either order), and each refusal names the stage that refused it (the
//! resolver names the host and the refused address). Each path has a control: a client built the
//! same way reaches an allowed destination, so the listeners are known to work.
//!
//! Every listener is on `127.0.0.1` (no loopback aliases needed). Host names resolve through the
//! policy's own resolver from fixed answers: `allowed.test` is allowed by name; `refused.test`,
//! `mixed-a.test` and `mixed-b.test` are not.

use std::collections::HashSet;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine as _;
use rssr_application::SubscriptionProbePort;
use rssr_infra::config_sync::webdav::WebDavConfigSync;
use rssr_infra::fetch::{BodyAssetLocalizer, FetchClient, FetchRequest};
use rssr_infra::outbound::{OutboundPolicy, PublicOnly};
use rssr_infra::subscription_probe::HttpSubscriptionProbe;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use url::Url;

const FEED: &str = r#"<?xml version="1.0"?><rss version="2.0"><channel><title>T</title><link>https://example.com/</link><item><title>A</title><link>https://example.com/a</link></item></channel></rss>"#;
const PNG: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=";

/// A listener answering every request with one fixed response, recording the peers it accepted.
struct Server {
    address: SocketAddr,
    accepted: Arc<Mutex<Vec<SocketAddr>>>,
    sentinels: Mutex<HashSet<SocketAddr>>,
}

impl Server {
    async fn start(response: Vec<u8>) -> Self {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.expect("bind a loopback listener");
        let address = listener.local_addr().unwrap();
        let accepted = Arc::new(Mutex::new(Vec::new()));
        let record = Arc::clone(&accepted);
        let response = Arc::new(response);
        tokio::spawn(async move {
            while let Ok((mut socket, peer)) = listener.accept().await {
                record.lock().unwrap().push(peer);
                let response = Arc::clone(&response);
                tokio::spawn(async move {
                    if read_request(&mut socket).await {
                        let _ = socket.write_all(&response).await;
                    }
                    let _ = socket.shutdown().await;
                });
            }
        });
        Self { address, accepted, sentinels: Mutex::new(HashSet::new()) }
    }

    /// The connections accepted so far, final for everything that reached the listener before
    /// this call: a sentinel connection is accepted after every earlier one in the queue.
    async fn settled_connections(&self) -> usize {
        let sentinel = TcpStream::connect(self.address).await.expect("connect a sentinel");
        let local = sentinel.local_addr().unwrap();
        self.sentinels.lock().unwrap().insert(local);
        for _ in 0..500 {
            if self.accepted.lock().unwrap().contains(&local) {
                let sentinels = self.sentinels.lock().unwrap();
                let accepted = self.accepted.lock().unwrap();
                return accepted.iter().filter(|peer| !sentinels.contains(peer)).count();
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("the sentinel connection was never accepted");
    }

    fn port(&self) -> u16 {
        self.address.port()
    }
}

/// Reads one request (headers, and a body of the declared length); false if the peer went away.
async fn read_request(socket: &mut TcpStream) -> bool {
    let mut request = Vec::new();
    let mut buffer = [0u8; 4096];
    let header_end = loop {
        if let Some(end) = request.windows(4).position(|window| window == b"\r\n\r\n") {
            break end + 4;
        }
        match socket.read(&mut buffer).await {
            Ok(0) | Err(_) => return false,
            Ok(count) => request.extend_from_slice(&buffer[..count]),
        }
    };
    let headers = String::from_utf8_lossy(&request[..header_end]).to_ascii_lowercase();
    let length = headers
        .lines()
        .find_map(|line| line.strip_prefix("content-length:"))
        .and_then(|value| value.trim().parse::<usize>().ok())
        .unwrap_or(0);
    while request.len() < header_end + length {
        match socket.read(&mut buffer).await {
            Ok(0) | Err(_) => return false,
            Ok(count) => request.extend_from_slice(&buffer[..count]),
        }
    }
    true
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

/// A localizer constructor under a policy (background or reader).
type NewLocalizer = fn(OutboundPolicy) -> anyhow::Result<BodyAssetLocalizer>;

/// The policy's own refusal, at the expected stage (`reason`), not some other failure: a resolver
/// that dropped the refused answers and connected to the public one would fail later, or be
/// refused at a redirect, for another reason.
fn assert_policy_refusal(error: &anyhow::Error, url: &str, reason: &str) {
    let chain = format!("{error:#}");
    assert!(chain.contains(&format!("出站访问策略不允许访问 {reason}")), "{url}: {chain}");
}

fn feed_response() -> Vec<u8> {
    response("200 OK", &[("Content-Type", "application/rss+xml".to_owned())], FEED.as_bytes())
}

fn loopback() -> IpAddr {
    "127.0.0.1".parse().unwrap()
}

/// `allowed.test` allowed by name; every other name resolves (in part) to loopback and is refused.
fn policy() -> OutboundPolicy {
    let public: IpAddr = "93.184.215.14".parse().unwrap();
    OutboundPolicy::PublicOnly(
        PublicOnly::new(vec!["allowed.test".to_owned()], Vec::new(), Vec::new())
            .with_fixed_answers("allowed.test", vec![loopback()])
            .with_fixed_answers("refused.test", vec![loopback()])
            .with_fixed_answers("mixed-a.test", vec![loopback(), public])
            .with_fixed_answers("mixed-b.test", vec![public, loopback()]),
    )
}

/// A refused target, a redirector (reached as `allowed.test`) pointing at it, and an allowed
/// server with the same content.
struct Destinations {
    target: Server,
    redirector: Server,
    allowed: Server,
}

impl Destinations {
    async fn start(body: Vec<u8>, path: &str) -> Self {
        let target = Server::start(body.clone()).await;
        let to_target = format!("http://127.0.0.1:{}{path}", target.port());
        let redirector =
            Server::start(response("302 Found", &[("Location", to_target)], b"")).await;
        let allowed = Server::start(body).await;
        Self { target, redirector, allowed }
    }

    /// Every way to reach the target that the policy must refuse, with the refusal it must give:
    /// the literal address (before sending, or at the redirect), or the resolver's host and
    /// refused address.
    fn refused_urls(&self, path: &str) -> Vec<(String, String)> {
        let target = self.target.port();
        let literal = format!("http://127.0.0.1:{target}");
        vec![
            (format!("{literal}{path}"), literal.clone()),
            (format!("http://allowed.test:{}{path}", self.redirector.port()), literal),
            (format!("http://refused.test:{target}{path}"), "refused.test（127.0.0.1）".to_owned()),
            (format!("http://mixed-a.test:{target}{path}"), "mixed-a.test（127.0.0.1）".to_owned()),
            (format!("http://mixed-b.test:{target}{path}"), "mixed-b.test（127.0.0.1）".to_owned()),
        ]
    }

    fn allowed_url(&self, path: &str) -> String {
        format!("http://allowed.test:{}{path}", self.allowed.port())
    }

    /// After the refused attempts: the target saw nothing, the redirector was asked once.
    async fn assert_refused(&self) {
        assert_eq!(self.target.settled_connections().await, 0, "the target was reached");
        assert_eq!(self.redirector.settled_connections().await, 1, "the redirector was asked");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn feed_fetching_reaches_only_allowed_destinations() {
    let at = Destinations::start(feed_response(), "/feed.xml").await;
    let request = |url: String| FetchRequest { url, etag: None, last_modified: None };
    let client = FetchClient::with_policy(policy()).unwrap();
    for (url, reason) in at.refused_urls("/feed.xml") {
        let error = client.fetch(&request(url.clone())).await.expect_err(&url);
        assert_policy_refusal(&error, &url, &reason);
    }
    at.assert_refused().await;
    client.fetch(&request(at.allowed_url("/feed.xml"))).await.expect("allowed by name");
    assert_eq!(at.allowed.settled_connections().await, 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn subscription_probing_reaches_only_allowed_destinations() {
    let at = Destinations::start(feed_response(), "/feed.xml").await;
    let probe = HttpSubscriptionProbe::with_policy(policy()).unwrap();
    for (url, reason) in at.refused_urls("/feed.xml") {
        let error = probe.probe(&Url::parse(&url).unwrap()).await.expect_err(&url);
        assert_policy_refusal(&error, &url, &reason);
    }
    at.assert_refused().await;
    let allowed = Url::parse(&at.allowed_url("/feed.xml")).unwrap();
    probe.probe(&allowed).await.expect("allowed by name");
    assert_eq!(at.allowed.settled_connections().await, 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn image_localization_reaches_only_allowed_destinations() {
    let png = base64::engine::general_purpose::STANDARD.decode(PNG).unwrap();
    let image = response("200 OK", &[("Content-Type", "image/png".to_owned())], &png);
    let localized = |localizer: BodyAssetLocalizer, src: String| async move {
        let html = format!(r#"<p><img src="{src}"></p>"#);
        localizer.localize_html_images(&html, None).await.unwrap().contains("data:image/png")
    };
    let localizers: [(&str, NewLocalizer); 2] =
        [("background", BodyAssetLocalizer::background), ("reader", BodyAssetLocalizer::reader)];
    for (kind, localizer) in localizers {
        let at = Destinations::start(image.clone(), "/a.png").await;
        for (url, _) in at.refused_urls("/a.png") {
            assert!(!localized(localizer(policy()).unwrap(), url.clone()).await, "{kind}: {url}");
        }
        at.assert_refused().await;
        assert!(localized(localizer(policy()).unwrap(), at.allowed_url("/a.png")).await, "{kind}");
        assert_eq!(at.allowed.settled_connections().await, 1, "{kind}");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn webdav_download_and_upload_reach_only_allowed_destinations() {
    let config = response("200 OK", &[("Content-Type", "application/json".to_owned())], b"{}");
    for upload in [false, true] {
        let at = Destinations::start(config.clone(), "/dav/config.json").await;
        let attempt = |endpoint: String| async move {
            let sync = WebDavConfigSync::with_policy(endpoint, "config.json", policy())?;
            if upload {
                sync.upload_text("{}").await.map(|()| None)
            } else {
                sync.download_text().await
            }
        };
        for (url, reason) in at.refused_urls("/dav/") {
            let error = attempt(url.clone()).await.expect_err(&url);
            assert_policy_refusal(&error, &format!("upload {upload}: {url}"), &reason);
        }
        at.assert_refused().await;
        attempt(at.allowed_url("/dav/")).await.expect("allowed by name");
        assert_eq!(at.allowed.settled_connections().await, 1, "upload {upload}");
    }
}

/// Every form a URL can give a literal loopback address in (as the URL parser reads it), a user
/// name that looks like a host, and a trailing-dot host name: refused by the policy under
/// `PublicOnly`. The unrestricted client reaches the listener through each IPv4 form; the
/// IPv4-mapped form has no such control (on Windows an IPv6 socket does not reach IPv4).
#[tokio::test(flavor = "multi_thread")]
async fn literal_address_forms_are_refused_and_reach_the_listener_when_unrestricted() {
    let target = Server::start(feed_response()).await;
    let port = target.port();
    let reachable = [
        format!("http://2130706433:{port}/feed.xml"),
        format!("http://0x7f.0.0.1:{port}/feed.xml"),
        format!("http://0177.0.0.1:{port}/feed.xml"),
        format!("http://127.1:{port}/feed.xml"),
        format!("http://%31%32%37.0.0.1:{port}/feed.xml"),
        format!("http://example.com@127.0.0.1:{port}/feed.xml"),
    ];
    let literal = format!("http://127.0.0.1:{port}");
    let refused_only = [
        (
            format!("http://[::ffff:127.0.0.1]:{port}/feed.xml"),
            format!("http://[::ffff:7f00:1]:{port}"),
        ),
        (format!("http://refused.test.:{port}/feed.xml"), "refused.test".to_owned()),
    ];
    let request = |url: &String| FetchRequest { url: url.clone(), etag: None, last_modified: None };

    let client = FetchClient::with_policy(policy()).unwrap();
    let refused = reachable.iter().map(|url| (url.clone(), literal.clone())).chain(refused_only);
    for (url, reason) in refused {
        let error = client.fetch(&request(&url)).await.expect_err(&url);
        assert_policy_refusal(&error, &url, &reason);
    }
    assert_eq!(target.settled_connections().await, 0);

    let allowed = Server::start(feed_response()).await;
    let named = format!("http://allowed.test.:{}/feed.xml", allowed.port());
    client.fetch(&request(&named)).await.expect("a trailing-dot allowed name is allowed");
    assert_eq!(allowed.settled_connections().await, 1);

    let unrestricted = FetchClient::new();
    for url in &reachable {
        unrestricted.fetch(&request(url)).await.unwrap_or_else(|error| panic!("{url}: {error:#}"));
    }
    assert_eq!(target.settled_connections().await, reachable.len());
}

/// A refusal reached through a redirect keeps the previous hop's credentials and token out of
/// the whole error chain (reqwest attaches that URL to a redirect error).
#[tokio::test(flavor = "multi_thread")]
async fn refusals_keep_credentials_out_of_the_error_chain() {
    let at = Destinations::start(feed_response(), "/feed.xml").await;
    let secret_url = format!(
        "http://user:SECRET-PASSWORD@allowed.test:{}/feed.xml?token=SECRET-TOKEN",
        at.redirector.port()
    );
    let request = FetchRequest { url: secret_url.clone(), etag: None, last_modified: None };
    let feed = FetchClient::with_policy(policy()).unwrap().fetch(&request).await.unwrap_err();
    let probe = HttpSubscriptionProbe::with_policy(policy()).unwrap();
    let probed = probe.probe(&Url::parse(&secret_url).unwrap()).await.unwrap_err();
    for error in [feed, probed] {
        let chain = format!("{error:#}");
        assert!(chain.contains("出站访问策略不允许访问"), "{chain}");
        assert!(!chain.contains("SECRET"), "{chain}");
    }
}

/// Subscription discovery: a page on an allowed host that declares a feed elsewhere cannot lead
/// the probe to a refused destination; a declared feed on an allowed host is subscribed.
#[tokio::test(flavor = "multi_thread")]
async fn discovered_candidates_are_checked_like_any_request() {
    use rssr_application::AddSubscriptionInput;
    use rssr_infra::composition::compose_library_use_cases_with;
    use rssr_infra::store::LibraryStore;

    let at = Destinations::start(feed_response(), "/feed.xml").await;
    let page_declaring = |feed: &str| {
        let html = format!(
            r#"<html><head><link rel="alternate" type="application/rss+xml" href="{feed}"></head><body>page</body></html>"#
        );
        response("200 OK", &[("Content-Type", "text/html".to_owned())], html.as_bytes())
    };
    let add = |page_port: u16| async move {
        let store = LibraryStore::open_in_memory().await.unwrap();
        let use_cases = compose_library_use_cases_with(store, &policy()).unwrap().use_cases;
        let input = AddSubscriptionInput {
            url: format!("http://allowed.test:{page_port}/"),
            title: None,
            folder: None,
        };
        use_cases.subscription_workflow.add_subscription(&input).await
    };

    let target = at.target.port();
    for (candidate, reason) in [
        (format!("http://127.0.0.1:{target}/feed.xml"), format!("http://127.0.0.1:{target}")),
        (format!("http://refused.test:{target}/feed.xml"), "refused.test（127.0.0.1）".to_owned()),
        (format!("http://mixed-a.test:{target}/feed.xml"), "mixed-a.test（127.0.0.1）".to_owned()),
        (format!("http://mixed-b.test:{target}/feed.xml"), "mixed-b.test（127.0.0.1）".to_owned()),
    ] {
        let page = Server::start(page_declaring(&candidate)).await;
        let error = add(page.port()).await.expect_err(&candidate);
        assert_policy_refusal(&error, &candidate, &reason);
        assert_eq!(page.settled_connections().await, 1, "the allowed page was read");
    }
    assert_eq!(at.target.settled_connections().await, 0, "no candidate reached the target");

    let page = Server::start(page_declaring(&at.allowed_url("/feed.xml"))).await;
    add(page.port()).await.expect("a declared feed on an allowed host");
    assert_eq!(at.allowed.settled_connections().await, 1);
}
