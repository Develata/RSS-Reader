//! Test-only upstream injection: the real bounded reader and response builder are used.
//! No fixture route or network-policy override is compiled into rssr-web.
use std::{collections::HashMap, io::Write, sync::Arc};

use axum::{Router, extract::Query, http::StatusCode, middleware, routing::get};
use reqwest::{ResponseBuilderExt, Url};
use tokio::sync::Mutex;
use tower_http::services::{ServeDir, ServeFile};

use super::{FeedProxyQuery, MAX_PROXIED_FEED_BYTES, build_proxy_response, feed_proxy};
use crate::auth::{AppState, handle_login, load_config, require_auth, session_probe, show_login};

const PROBE: &str = "window.__proxyExecuted = true; window.__proxyRead = localStorage.getItem('rssr-proxy-sentinel'); localStorage.setItem('rssr-proxy-read', window.__proxyRead); localStorage.setItem('rssr-proxy-sentinel', 'changed-by-fixture');";
const LAST_MODIFIED: &str = "Wed, 07 Oct 2026 00:00:00 GMT";

fn fixture(url: &Url) -> Option<reqwest::Response> {
    if url.host_str() != Some("fixture.example.invalid") || url.scheme() != "https" {
        return None;
    }
    let mut status = StatusCode::OK;
    let mut final_url = url.clone();
    let (mime, body) = match url.path() {
        "/html" | "/error" | "/sniff" => {
            if url.path() == "/error" {
                status = StatusCode::NOT_FOUND;
            }
            let mime = (url.path() != "/sniff").then_some("text/html; charset=utf-8");
            (mime, format!("<!doctype html><title>Proxy fixture</title><script>{PROBE}</script>").into_bytes())
        }
        "/svg" | "/xml" => (
            Some(if url.path() == "/svg" { "image/svg+xml" } else { "application/xml" }),
            format!("<svg xmlns=\"http://www.w3.org/2000/svg\"><script>{PROBE}</script></svg>").into_bytes(),
        ),
        "/xhtml" => (
            Some("application/xhtml+xml"),
            format!("<html xmlns=\"http://www.w3.org/1999/xhtml\"><head><title>Fixture</title></head><body><script>{PROBE}</script></body></html>").into_bytes(),
        ),
        "/rss.xml" | "/moved/rss.xml" | "/latin.xml" => {
            let title = match url.path() {
                "/moved/rss.xml" => "Discovered Feed",
                "/latin.xml" => "Caf\u{e9} Feed",
                _ => "RSS Fixture",
            };
            let xml = format!("<rss version=\"2.0\"><channel><title>{title}</title><link>https://fixture.example.invalid/</link><description>Fixture</description><item><guid>{title}</guid><title>{title} Entry</title><description>Harmless fixture body</description></item></channel></rss>");
            if url.path() == "/latin.xml" {
                (Some("application/rss+xml; charset=iso-8859-1"), xml.chars().map(|c| c as u8).collect())
            } else {
                (Some("application/rss+xml; charset=utf-8"), xml.into_bytes())
            }
        }
        "/atom.xml" => (Some("application/atom+xml; charset=utf-8"), b"<feed xmlns=\"http://www.w3.org/2005/Atom\"><title>Atom Fixture</title><id>urn:fixture:atom</id><updated>2026-10-07T00:00:00Z</updated><entry><id>urn:fixture:entry</id><title>Atom Fixture Entry</title><updated>2026-10-07T00:00:00Z</updated><content>Harmless fixture body</content></entry></feed>".to_vec()),
        "/landing" => {
            final_url.set_path("/moved/index.html");
            // MIME is necessary: no leading html/doctype sniff, and a relative discovery URL.
            (Some("text/html; charset=utf-8"), format!("<!-- {} --><head><link rel=\"alternate\" type=\"application/rss+xml\" href=\"rss.xml\" title=\"Discovered Feed\"></head><script>{PROBE}</script>", "fixture ".repeat(40)).into_bytes())
        }
        "/not-modified" => {
            status = StatusCode::NOT_MODIFIED;
            (Some("application/rss+xml"), Vec::new())
        }
        "/oversized" => (Some("text/html"), vec![b'x'; MAX_PROXIED_FEED_BYTES + 1]),
        _ => return None,
    };
    let mut response = axum::http::Response::builder()
        .status(status)
        .url(final_url)
        .header("etag", "\"fixture-v1\"")
        .header("last-modified", LAST_MODIFIED)
        // Upstream policy must never weaken the application's policy.
        .header("content-security-policy", "sandbox allow-scripts allow-same-origin")
        .header("set-cookie", "untrusted=1");
    if let Some(mime) = mime {
        response = response.header("content-type", mime);
    }
    Some(response.body(body).unwrap().into())
}

#[tokio::test]
async fn responses_isolate_untrusted_content_without_changing_feed_data() {
    for case in [
        "html",
        "svg",
        "xml",
        "xhtml",
        "sniff",
        "error",
        "rss.xml",
        "atom.xml",
        "latin.xml",
        "landing",
        "not-modified",
    ] {
        let url = Url::parse(&format!("https://fixture.example.invalid/{case}")).unwrap();
        let source = fixture(&url).unwrap();
        let expected_status = source.status();
        let expected_mime = source.headers().get("content-type").cloned();
        let expected_url = source.url().to_string();
        let expected_bytes = source.bytes().await.unwrap();
        let actual = build_proxy_response(fixture(&url).unwrap()).await;
        assert_eq!(actual.status(), expected_status, "{case}");
        assert_eq!(actual.headers().get("content-type"), expected_mime.as_ref(), "{case}");
        assert_eq!(actual.headers()["x-rssr-final-url"], expected_url, "{case}");
        assert_eq!(actual.headers()["etag"], "\"fixture-v1\"");
        assert_eq!(actual.headers()["last-modified"], LAST_MODIFIED);
        assert!(!actual.headers().contains_key("set-cookie"));
        assert_eq!(
            actual.headers().get("content-security-policy").and_then(|h| h.to_str().ok()),
            Some("sandbox; default-src 'none'; base-uri 'none'; form-action 'none'"),
            "{case}: untrusted documents must be sandboxed without exceptions"
        );
        assert_eq!(
            actual.headers().get("x-content-type-options").and_then(|h| h.to_str().ok()),
            Some("nosniff")
        );
        let bytes = axum::body::to_bytes(actual.into_body(), MAX_PROXIED_FEED_BYTES).await.unwrap();
        assert_eq!(bytes, expected_bytes, "{case}");
    }
}

#[tokio::test]
async fn response_body_limit_remains_enforced() {
    let url = Url::parse("https://fixture.example.invalid/oversized").unwrap();
    let response = build_proxy_response(fixture(&url).unwrap()).await;
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
}

/// Started only by scripts/browser/feed_proxy_isolation.mjs in an isolated process.
#[tokio::test]
#[ignore = "requires the isolated CDP browser driver and an explicit temporary auth path"]
async fn browser_fixture_server() {
    assert!(std::env::var_os("RSSR_PROXY_ISOLATION_TEST").is_some());
    assert!(std::env::var_os("RSS_READER_WEB_AUTH_STATE_FILE").is_some());
    let config = Arc::new(load_config().unwrap());
    let state =
        AppState { config: config.clone(), login_throttle: Arc::new(Mutex::new(HashMap::new())) };
    let protected = Router::new()
        .route("/session-probe", get(session_probe))
        .route(
            "/feed-proxy",
            get(|Query(query): Query<FeedProxyQuery>| async move {
                if let Ok(url) = Url::parse(&query.url)
                    && let Some(response) = fixture(&url)
                {
                    build_proxy_response(response).await
                } else {
                    // Real production rejection path; SSRF/DNS policy is never relaxed.
                    axum::response::IntoResponse::into_response(feed_proxy(Query(query)).await)
                }
            }),
        )
        .route(
            "/control",
            get(|Query(query): Query<FeedProxyQuery>| async move {
                let response = fixture(&Url::parse(&query.url).unwrap()).unwrap();
                let mime = response.headers().get("content-type").cloned().unwrap();
                axum::http::Response::builder()
                    .header("content-type", mime)
                    .body(axum::body::Body::from(response.bytes().await.unwrap()))
                    .unwrap()
            }),
        )
        .route(
            "/sentinel",
            get(|| async {
                axum::response::Html("<!doctype html><title>Isolated test origin</title>")
            }),
        )
        .fallback_service(
            ServeDir::new(&config.static_dir)
                .not_found_service(ServeFile::new(config.static_dir.join("index.html"))),
        )
        .layer(middleware::from_fn_with_state(state.clone(), require_auth));
    let app = Router::new()
        .route("/login", get(show_login).post(handle_login))
        .merge(protected)
        .with_state(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    println!("RSSR_PROXY_TEST_URL=http://{}", listener.local_addr().unwrap());
    std::io::stdout().flush().unwrap();
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            tokio::task::spawn_blocking(|| {
                let _ = std::io::stdin().read_line(&mut String::new());
            })
            .await
            .unwrap();
        })
        .await
        .unwrap();
}
