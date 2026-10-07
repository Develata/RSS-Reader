#![cfg(not(target_arch = "wasm32"))]

#[path = "support/request_capture.rs"]
mod request_capture;

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use request_capture::{Response, Server};
use rssr_infra::{config_sync::webdav::WebDavConfigSync, fetch::BodyAssetLocalizer};
use url::Url;

const CONFIG: &str = r#"{"feeds":[],"settings":{"theme":"dark"}}"#;

fn redirect(status: u16, location: &str) -> Response {
    Response::new(status, &format!("Location: {location}\r\n"), [])
}

async fn localize(url: &Url, article: Option<&Url>) -> String {
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        BodyAssetLocalizer::new().localize_html_images(&format!(r#"<img src="{url}">"#), article),
    )
    .await
    .expect("bounded fixture request")
    .expect("localize")
}

fn authenticated_endpoint(url: &Url) -> String {
    let mut endpoint = url.clone();
    endpoint.set_username("fixture%2540user").unwrap();
    endpoint.set_password(Some("fake%253Apassword")).unwrap();
    endpoint.to_string()
}

#[tokio::test]
async fn image_initial_referer_contains_only_article_origin() {
    let image = Server::start(|_| Response::new(200, "Content-Type: image/png\r\n", b"png")).await;
    let article = Url::parse("http://fake-user:fake-password@example.test:80/private/article?token=article-secret#fragment-secret").unwrap();
    assert!(localize(&image.url, Some(&article)).await.contains("data:image/png;base64,"));
    let requests = image.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].header("referer"), Some("http://example.test/"));
}

#[tokio::test]
async fn image_redirects_never_rebuild_referer_from_signed_image_url() {
    let destination =
        Server::start(|_| Response::new(200, "Content-Type: image/png\r\n", b"png")).await;
    let next = destination.url.to_string();
    let source = Server::start(move |request| {
        if request.path.starts_with("/first") {
            redirect(302, "/second?signature=second-secret")
        } else {
            redirect(307, &next)
        }
    })
    .await;
    let article = Url::parse("http://example.test/").unwrap();
    let image = source.url.join("first?signature=first-secret").unwrap();
    for article in [Some(&article), None] {
        assert!(localize(&image, article).await.contains("data:image/png;base64,"));
    }
    let initial = source.requests();
    let final_hop = destination.requests();
    assert_eq!((initial.len(), final_hop.len()), (4, 2));
    for request in initial.iter().take(2).chain(final_hop.iter().take(1)) {
        assert_eq!(request.header("referer"), Some("http://example.test/"));
    }
    for request in initial.iter().skip(2).chain(final_hop.iter().skip(1)) {
        assert_eq!(request.header("referer"), None);
    }
}

#[tokio::test]
async fn image_https_article_sends_no_referer_to_initial_http_image() {
    let image = Server::start(|_| Response::new(200, "Content-Type: image/png\r\n", b"png")).await;
    let article = Url::parse("https://example.test/private?token=article-secret").unwrap();
    assert!(localize(&image.url, Some(&article)).await.contains("data:image/png;base64,"));
    assert_eq!(image.requests()[0].header("referer"), None);
}

#[tokio::test]
async fn image_rejects_redirect_userinfo_before_contacting_target() {
    let target = Server::start(|_| Response::new(200, "Content-Type: image/png\r\n", b"png")).await;
    let location = authenticated_endpoint(&target.url);
    let source = Server::start(move |_| redirect(302, &location)).await;
    assert!(!localize(&source.url, None).await.contains("data:image/"));
    assert_eq!(source.requests().len(), 1);
    assert!(target.requests().is_empty());
}

#[tokio::test]
async fn image_redirect_loop_is_bounded() {
    let source = Server::start(|_| redirect(302, "/loop?signature=loop-secret")).await;
    let html = localize(&source.url, None).await;
    assert!(!html.contains("data:image/"));
    assert!(html.contains(&format!(r#"src="{}""#, source.url)));
    // reqwest 0.12.28 allows ten redirects after the initial request. The public
    // localizer absorbs the error; exact count rules out an unrelated early failure.
    assert_eq!(source.requests().len(), 11);
}

#[tokio::test]
async fn image_preserves_mime_and_background_size_limits() {
    let source = Server::start(|request| match request.path.as_str() {
        "/html" => Response::new(200, "Content-Type: text/html\r\n", b"not an image"),
        "/oversize" => {
            Response::new(200, "Content-Type: image/png\r\n", vec![0; 3 * 1024 * 1024 + 1])
        }
        _ => Response::new(200, "Content-Type: image/png\r\n", b"png"),
    })
    .await;
    for path in ["html", "oversize"] {
        assert!(!localize(&source.url.join(path).unwrap(), None).await.contains("data:image/"));
    }
    assert!(
        localize(&source.url.join("valid").unwrap(), None).await.contains("data:image/png;base64,")
    );
}

#[tokio::test]
async fn webdav_normal_paths_keep_authentication_method_and_body() {
    let server = Server::start(|request| {
        if request.method == "PUT" {
            Response::new(201, "", [])
        } else {
            Response::new(200, "Content-Type: application/json\r\n", CONFIG)
        }
    })
    .await;
    for base in ["base", "base/"] {
        for path in [
            "config%2Fstate%3Fname%23x.json?version=1#fragment",
            "/config%2Fstate%3Fname%23x.json?version=1#fragment",
        ] {
            let sync = WebDavConfigSync::new(
                authenticated_endpoint(&server.url.join(base).unwrap()),
                path,
            )
            .unwrap();
            sync.upload_text(CONFIG).await.unwrap();
            assert_eq!(sync.download_text().await.unwrap().as_deref(), Some(CONFIG));
        }
    }
    let requests = server.requests();
    assert_eq!(requests.len(), 8);
    let expected_auth = format!("Basic {}", BASE64.encode("fixture%40user:fake%3Apassword"));
    for pair in requests.as_chunks::<2>().0 {
        assert_eq!(pair[0].method, "PUT");
        assert_eq!(pair[0].body, CONFIG.as_bytes());
        assert_eq!(pair[1].method, "GET");
        assert!(pair[1].body.is_empty());
        for request in pair {
            assert_eq!(request.path, "/base/config%2Fstate%3Fname%23x.json?version=1");
            assert_eq!(request.header("authorization"), Some(expected_auth.as_str()));
        }
    }
}

#[tokio::test]
async fn webdav_rejected_paths_contact_neither_server() {
    let endpoint = Server::start(|_| Response::new(200, "", CONFIG)).await;
    let target = Server::start(|_| Response::new(200, "", CONFIG)).await;
    let authority = target.url[url::Position::BeforeHost..url::Position::AfterPort].to_owned();
    for path in [
        target.url.to_string(),
        endpoint.url.to_string(),
        format!("//{authority}/state"),
        format!("\\\\{authority}/state"),
        format!("/\\{authority}/state"),
        format!(" /{}/{authority}/state", "/"),
        format!("\t{}", target.url),
        format!("h\nttp://{authority}/state"),
        format!("/{}/state", target.url),
    ] {
        let sync =
            WebDavConfigSync::new(authenticated_endpoint(&endpoint.url), path.clone()).unwrap();
        let put = sync.upload_text(CONFIG).await;
        let get = sync.download_text().await;
        assert!(put.is_err() && get.is_err(), "accepted unsafe path {path:?}");
    }
    assert!(endpoint.requests().is_empty());
    assert!(target.requests().is_empty());
}

#[tokio::test]
async fn webdav_same_origin_redirects_preserve_put_and_get() {
    let expected_auth = format!("Basic {}", BASE64.encode("fixture%40user:fake%3Apassword"));
    // Locked reqwest/tower-http preserve PUT for 301/302 as well as 307/308.
    for status in [301, 302, 307, 308] {
        let server = Server::start(move |request| {
            if request.path.starts_with("/start") {
                redirect(status, "/final?signature=synthetic")
            } else if request.method == "PUT" {
                Response::new(201, "", [])
            } else {
                Response::new(200, "", CONFIG)
            }
        })
        .await;
        let sync = WebDavConfigSync::new(authenticated_endpoint(&server.url), "start").unwrap();
        sync.upload_text(CONFIG).await.unwrap();
        assert_eq!(sync.download_text().await.unwrap().as_deref(), Some(CONFIG));
        let requests = server.requests();
        assert_eq!(requests.len(), 4);
        for request in &requests[..2] {
            assert_eq!(request.method, "PUT");
            assert_eq!(request.body, CONFIG.as_bytes());
        }
        for request in &requests[2..] {
            assert_eq!(request.method, "GET");
        }
        for request in &requests {
            assert_eq!(request.header("authorization"), Some(expected_auth.as_str()));
            assert_eq!(request.header("referer"), None);
        }
    }
}

#[tokio::test]
async fn webdav_cross_origin_redirects_never_send_credentials_or_config() {
    for status in [301, 302, 303, 307, 308] {
        let target = Server::start(|_| Response::new(200, "", CONFIG)).await;
        let location = target.url.join("secret-path?token=redirect-secret").unwrap().to_string();
        let endpoint = Server::start(move |_| redirect(status, &location)).await;
        let sync = WebDavConfigSync::new(
            authenticated_endpoint(&endpoint.url),
            "state?token=request-secret",
        )
        .unwrap();
        for error in
            [sync.upload_text(CONFIG).await.unwrap_err(), sync.download_text().await.unwrap_err()]
        {
            let text = format!("{error:?} {error:#}");
            for secret in ["fixture", "fake", "secret-path", "request-secret", "redirect-secret"] {
                assert!(!text.contains(secret), "URL secret leaked in error: {text}");
            }
        }
        assert_eq!(endpoint.requests().len(), 2);
        assert!(target.requests().is_empty());
    }
}

#[tokio::test]
async fn webdav_method_changing_redirect_cannot_report_successful_upload() {
    let server = Server::start(|request| {
        if request.path == "/start" {
            redirect(303, "/final")
        } else {
            Response::new(200, "", CONFIG)
        }
    })
    .await;
    let sync = WebDavConfigSync::new(authenticated_endpoint(&server.url), "start").unwrap();
    assert!(sync.upload_text(CONFIG).await.is_err(), "accepted PUT redirect 303");
    assert_eq!(server.requests().len(), 1);
}

#[tokio::test]
async fn webdav_redirect_userinfo_is_rejected_even_on_same_origin() {
    let target = Server::start(|_| Response::new(200, "", CONFIG)).await;
    let location = authenticated_endpoint(&target.url);
    let source = Server::start(move |request| {
        if request.path == "/same" {
            redirect(307, &format!("http://fake:secret@{}/final", request.header("host").unwrap()))
        } else {
            redirect(308, &location)
        }
    })
    .await;
    for path in ["same", "cross"] {
        let sync = WebDavConfigSync::new(source.url.as_str(), path).unwrap();
        assert!(sync.upload_text(CONFIG).await.is_err());
        assert!(sync.download_text().await.is_err());
    }
    assert_eq!(source.requests().len(), 4);
    assert!(target.requests().is_empty());
}

#[tokio::test]
async fn webdav_redirect_loop_is_bounded_and_errors_omit_url_secrets() {
    let server = Server::start(|_| redirect(307, "/loop?token=loop-secret")).await;
    let sync =
        WebDavConfigSync::new(authenticated_endpoint(&server.url), "start?token=request-secret")
            .unwrap();
    let error = sync.download_text().await.unwrap_err();
    // The initial request plus ten followed redirects; reject the next attempt.
    assert_eq!(server.requests().len(), 11);
    let request_error = error.downcast_ref::<reqwest::Error>().expect("reqwest request error");
    assert!(request_error.is_redirect(), "expected redirect exhaustion: {error:#}");
    assert_eq!(error.root_cause().to_string(), "too many redirects");
    let text = format!("{error:?} {error:#}");
    assert!(!text.contains("request-secret") && !text.contains("loop-secret"), "{text}");
}

#[tokio::test]
async fn webdav_download_keeps_four_mib_limit_and_not_found_semantics() {
    let server = Server::start(|request| match request.path.as_str() {
        "/exact" => Response::new(200, "", vec![b'x'; 4 * 1024 * 1024]),
        "/oversize" => Response::new(200, "", vec![b'x'; 4 * 1024 * 1024 + 1]),
        _ => Response::new(404, "", []),
    })
    .await;
    let sync = |path| WebDavConfigSync::new(server.url.as_str(), path).unwrap();
    assert_eq!(sync("exact").download_text().await.unwrap().unwrap().len(), 4 * 1024 * 1024);
    assert!(sync("oversize").download_text().await.is_err());
    assert_eq!(sync("missing").download_text().await.unwrap(), None);
}
