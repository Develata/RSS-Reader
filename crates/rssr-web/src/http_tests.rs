//! The whole router over real HTTP (v0.2 phase C, C2): Origin and CSRF checks, the JSON API
//! boundary, login, logout and revocation, in the order the middleware actually runs.

use std::{net::SocketAddr, sync::Arc};

use reqwest::{Client, Response, StatusCode, header, redirect::Policy};

use crate::{
    app,
    auth::{
        AppState, AuthStore, CSRF_HEADER, generate_password_hash, parse_public_origin,
        test_support::{ScratchDir, sample_config, sample_store},
    },
};

struct Server {
    base: String,
    state: AppState,
    store: Arc<AuthStore>,
    client: Client,
    _dir: ScratchDir,
}

impl Server {
    async fn start(public_origin: Option<&str>) -> Self {
        let dir = ScratchDir::new("http");
        let static_dir = dir.path().join("public");
        std::fs::create_dir_all(&static_dir).unwrap();
        std::fs::write(static_dir.join("index.html"), "<!doctype html><title>app shell</title>")
            .unwrap();
        let store = sample_store(&dir);
        let mut config = sample_config();
        config.static_dir = static_dir;
        config.public_origin = public_origin.map(|raw| parse_public_origin(raw).unwrap());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let local = listener.local_addr().unwrap();
        let state = AppState::new(Arc::new(config), Arc::clone(&store), local).unwrap();
        let service = app(state.clone()).into_make_service_with_connect_info::<SocketAddr>();
        tokio::spawn(async move { axum::serve(listener, service).await.unwrap() });
        let client = Client::builder().redirect(Policy::none()).no_proxy().build().unwrap();
        Self { base: format!("http://{local}"), state, store, client, _dir: dir }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }

    async fn login_from(&self, origin: Option<&str>, password: &str) -> Response {
        let mut request = self.client.post(self.url("/login")).form(&[
            ("username", "demo"),
            ("password", password),
            ("next", "/feeds"),
        ]);
        if let Some(origin) = origin {
            request = request.header(header::ORIGIN, origin);
        }
        request.send().await.unwrap()
    }

    /// Logs in from the trusted origin; returns the `Cookie` header value of the session.
    async fn session(&self) -> String {
        let response = self.login_from(Some(&self.base), "secret123").await;
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        cookie(&response, "rssr_web_session").expect("a session cookie")
    }

    async fn get(&self, path: &str, cookie: Option<&str>) -> Response {
        let mut request = self.client.get(self.url(path));
        if let Some(cookie) = cookie {
            request = request.header(header::COOKIE, cookie);
        }
        request.send().await.unwrap()
    }

    async fn csrf(&self, cookie: &str) -> String {
        let response = self.get("/api/v1/session", Some(cookie)).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body: serde_json::Value = response.json().await.unwrap();
        body["csrf_token"].as_str().unwrap().to_owned()
    }
}

/// `name=value` of the `Set-Cookie` named `name`.
fn cookie(response: &Response, name: &str) -> Option<String> {
    set_cookie(response, name).map(|line| line.split(';').next().unwrap().to_owned())
}

fn set_cookie(response: &Response, name: &str) -> Option<String> {
    response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .find(|line| line.starts_with(&format!("{name}=")))
        .map(str::to_owned)
}

fn location(response: &Response) -> &str {
    response.headers()[header::LOCATION].to_str().unwrap()
}

async fn json_code(response: Response) -> String {
    let content_type = response.headers()[header::CONTENT_TYPE].to_str().unwrap().to_owned();
    assert!(content_type.starts_with("application/json"), "{content_type}");
    let body: serde_json::Value = response.json().await.unwrap();
    body["code"].as_str().unwrap().to_owned()
}

/// A write request needs exactly a trusted origin: missing, `null`, another port of the same
/// host, another site and https on the same host are refused before any password is checked.
#[tokio::test]
async fn writes_need_exactly_a_trusted_origin() {
    let server = Server::start(None).await;
    let port = server.base.rsplit(':').next().unwrap().to_owned();
    for origin in [
        None,
        Some("null".to_owned()),
        Some("http://127.0.0.1:1".to_owned()),
        Some("http://evil.example".to_owned()),
        Some(format!("https://127.0.0.1:{port}")),
    ] {
        let response = server.login_from(origin.as_deref(), "secret123").await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN, "{origin:?}");
        assert!(cookie(&response, "rssr_web_session").is_none(), "{origin:?}");
    }
    assert!(server.store.credential().unwrap().is_some());

    let response = server.login_from(Some(&server.base), "secret123").await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(location(&response), "/feeds");
    let session = set_cookie(&response, "rssr_web_session").unwrap();
    for attribute in ["Path=/", "HttpOnly", "SameSite=Lax", "Max-Age=15552000"] {
        assert!(session.contains(attribute), "{session}");
    }
    assert!(!session.contains("Secure"), "plain http on loopback: {session}");
    assert!(set_cookie(&response, "rssr_web_gate").unwrap().contains("Max-Age=15552000"));

    let localhost = format!("http://localhost:{port}");
    let response = server.login_from(Some(&localhost), "secret123").await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER, "the bound port under its name");
}

/// `/api` answers JSON and never falls back to the app: 401 without a session, 404 for an
/// unknown route with one. The session route is not cached and carries no CORS headers.
#[tokio::test]
async fn the_api_answers_json_and_never_falls_back_to_the_app() {
    let server = Server::start(None).await;
    for path in ["/api/v1/session", "/api/v1/nope", "/api", "/api/"] {
        let response = server.get(path, None).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{path}");
        assert_eq!(json_code(response).await, "unauthenticated", "{path}");
    }
    let response = server.get("/entries", None).await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(location(&response), "/login?next=%2Fentries");

    let session = server.session().await;
    let response = server
        .client
        .get(server.url("/api/v1/session"))
        .header(header::COOKIE, &session)
        .header(header::ORIGIN, "http://evil.example")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert!(
        !response.headers().keys().any(|name| name.as_str().starts_with("access-control-")),
        "{:?}",
        response.headers()
    );
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["csrf_token"].as_str().unwrap().len(), 43);

    let response = server.get("/api/v1/nope", Some(&session)).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(json_code(response).await, "not_found");
    // The app's fallback (index.html, with 404 for a missing file), not the API's JSON.
    for (path, status) in
        [("/entries", StatusCode::OK), ("/assets/missing.js", StatusCode::NOT_FOUND)]
    {
        let response = server.get(path, Some(&session)).await;
        assert_eq!(response.status(), status, "{path}");
        assert!(response.text().await.unwrap().contains("app shell"), "{path}");
    }
}

/// An API write needs a trusted origin and the session's CSRF token, in that order.
#[tokio::test]
async fn api_writes_need_the_session_csrf_token() {
    let server = Server::start(None).await;
    let session = server.session().await;
    let csrf = server.csrf(&session).await;
    let write = |origin: Option<&str>, token: Option<&str>| {
        let mut request =
            server.client.post(server.url("/api/v1/nope")).header(header::COOKIE, &session);
        if let Some(origin) = origin {
            request = request.header(header::ORIGIN, origin);
        }
        if let Some(token) = token {
            request = request.header(CSRF_HEADER, token);
        }
        request.send()
    };

    let response = write(None, Some(&csrf)).await.unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(json_code(response).await, "untrusted_origin");
    for token in [None, Some("wrong"), Some(&csrf[..42])] {
        let response = write(Some(&server.base), token).await.unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN, "{token:?}");
        assert_eq!(json_code(response).await, "csrf_mismatch");
    }
    let other = server.session().await;
    let response = write(Some(&server.base), Some(&server.csrf(&other).await)).await.unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN, "another session's token");
    let response = write(Some(&server.base), Some(&csrf)).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND, "reached the route");
}

/// `GET /logout` is a confirmation page; `POST /logout` needs the trusted origin and the CSRF
/// token, then revokes the session on the server: the old cookie replayed is refused.
#[tokio::test]
async fn logout_confirms_then_revokes_the_session() {
    let server = Server::start(None).await;
    let response = server.get("/logout", None).await;
    assert_eq!((response.status(), location(&response)), (StatusCode::SEE_OTHER, "/login"));

    let session = server.session().await;
    let csrf = server.csrf(&session).await;
    let response = server.get("/logout", Some(&session)).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let page = response.text().await.unwrap();
    assert!(page.contains(r#"<form method="post" action="/logout">"#), "{page}");
    assert!(page.contains(&format!(r#"name="csrf" value="{csrf}""#)), "{page}");

    let logout = |origin: &str, form: &[(&str, &str)]| {
        server
            .client
            .post(server.url("/logout"))
            .header(header::COOKIE, &session)
            .header(header::ORIGIN, origin)
            .form(form)
            .send()
    };
    let response = logout("http://evil.example", &[("csrf", &csrf)]).await.unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN, "a cross-site form");
    let response = logout(&server.base, &[]).await.unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN, "no token");
    assert_eq!(server.get("/api/v1/session", Some(&session)).await.status(), StatusCode::OK);

    let response = logout(&server.base, &[("csrf", &csrf)]).await.unwrap();
    assert_eq!((response.status(), location(&response)), (StatusCode::SEE_OTHER, "/login"));
    for name in ["rssr_web_session", "rssr_web_gate"] {
        assert!(set_cookie(&response, name).unwrap().contains("Max-Age=0"), "{name}");
    }
    let replayed = server.get("/api/v1/session", Some(&session)).await;
    assert_eq!(replayed.status(), StatusCode::UNAUTHORIZED);
    let replayed = server.get("/entries", Some(&session)).await;
    assert_eq!(location(&replayed), "/login?error=session_expired&next=%2Fentries");

    // The header carries the token as well (for scripts).
    let session = server.session().await;
    let csrf = server.csrf(&session).await;
    let response = server
        .client
        .post(server.url("/logout"))
        .header(header::COOKIE, &session)
        .header(header::ORIGIN, &server.base)
        .header(CSRF_HEADER, &csrf)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        server.get("/api/v1/session", Some(&session)).await.status(),
        StatusCode::UNAUTHORIZED
    );
}

/// A v0.1 HMAC session cookie is refused like an expired session.
#[tokio::test]
async fn a_legacy_session_cookie_is_refused() {
    let server = Server::start(None).await;
    let legacy = "rssr_web_session=ZGVtbzo5OTk5OTk5OTk5.c2lnbmF0dXJlLW9mLXRoZS1vbGQtc2VjcmV0";
    let response = server.get("/entries", Some(legacy)).await;
    assert_eq!(location(&response), "/login?error=session_expired&next=%2Fentries");
    let response = server.get("/api/v1/session", Some(legacy)).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

/// Changing the password revokes every session and the old password stops working;
/// revoking all sessions keeps the credential.
#[tokio::test]
async fn a_password_change_and_revoke_all_end_sessions() {
    let server = Server::start(None).await;
    let session = server.session().await;
    server.store.change_password(&generate_password_hash("new-secret-456").unwrap()).unwrap();
    assert_eq!(
        server.get("/api/v1/session", Some(&session)).await.status(),
        StatusCode::UNAUTHORIZED
    );
    let response = server.login_from(Some(&server.base), "secret123").await;
    assert!(location(&response).contains("error=invalid_credentials"));
    let response = server.login_from(Some(&server.base), "new-secret-456").await;
    assert_eq!(location(&response), "/feeds");

    let session = cookie(&response, "rssr_web_session").unwrap();
    assert_eq!(server.store.revoke_all().unwrap(), 1);
    assert_eq!(
        server.get("/api/v1/session", Some(&session)).await.status(),
        StatusCode::UNAUTHORIZED
    );
    let response = server.login_from(Some(&server.base), "new-secret-456").await;
    assert_eq!(location(&response), "/feeds", "the credential is kept");
}

/// With a public origin, only it is trusted (not the loopback address the server listens on),
/// and cookies are `Secure` when it is https.
#[tokio::test]
async fn a_public_origin_is_the_only_trusted_one() {
    let server = Server::start(Some("https://reader.example")).await;
    let response = server.login_from(Some(&server.base), "secret123").await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let response = server.login_from(Some("https://reader.example"), "secret123").await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    for name in ["rssr_web_session", "rssr_web_gate", "rssr_web_device"] {
        assert!(set_cookie(&response, name).unwrap().contains("; Secure"), "{name}");
    }
}

/// Session checks are bounded: with every slot taken, requests that carry a session cookie are
/// refused at once with 503 (JSON under /api), while requests without one still work.
#[tokio::test]
async fn session_checks_are_bounded() {
    let server = Server::start(None).await;
    let session = server.session().await;
    let busy = server.state.exhaust_session_checks();

    assert_eq!(
        server.get("/entries", Some(&session)).await.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    let response = server.get("/api/v1/session", Some(&session)).await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(json_code(response).await, "busy");
    assert_eq!(server.get("/login", None).await.status(), StatusCode::OK);
    let response = server.get("/entries", None).await;
    assert_eq!(location(&response), "/login?next=%2Fentries", "no cookie, no check needed");

    drop(busy);
    assert_eq!(server.get("/entries", Some(&session)).await.status(), StatusCode::OK);
}
