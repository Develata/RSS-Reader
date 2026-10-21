//! 服务端模式的 `/api/v1`：把 [`rssr_host::protocol`] 的端点暴露给浏览器端。
//!
//! 调用条件在读取请求体之前检查（`guard`）：有效会话（否则 401，不重定向到登录页）、恰好一个
//! `Content-Type: application/json`、恰好一个匹配的协议版本头，以及与服务地址同源的 `Origin`（若带）。
//! 自定义头与 JSON 类型让跨源请求必须先经预检，而本服务不返回任何 CORS 许可；`Origin` 校验是额外一层。
//!
//! 调用在独立任务里执行：浏览器断开或刷新页面不会中断进行中的刷新或写入。同时在途的调用数有上限，
//! 准入许可随任务持有到完成，饱和时返回 503。
use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, Request, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use rssr_host::{
    HostServices,
    protocol::{ApiError, CAPABILITIES_PATH, Capabilities, PROTOCOL_HEADER, PROTOCOL_VERSION},
    server::{DispatchError, dispatch},
};
use tokio::sync::Semaphore;

use crate::auth::{AuthConfig, request_has_valid_session};

/// 导入的配置包与 OPML 都是整段文本，放宽到 16 MiB。
const MAX_API_BODY_BYTES: usize = 16 * 1024 * 1024;
/// 读完请求体的期限。只对已通过守卫的请求生效，防止慢速上传长期占住连接与缓冲。
const BODY_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
/// 同时在途的接口调用上限。单用户的界面远用不到这么多；它只防失控的重试或滥用耗尽连接与内存。
const MAX_IN_FLIGHT_CALLS: usize = 32;

#[derive(Clone)]
pub(crate) struct ApiState {
    config: Arc<AuthConfig>,
    services: HostServices,
    admission: Arc<Semaphore>,
}

impl ApiState {
    pub(crate) fn new(config: Arc<AuthConfig>, services: HostServices) -> Self {
        Self::with_capacity(config, services, MAX_IN_FLIGHT_CALLS)
    }

    fn with_capacity(config: Arc<AuthConfig>, services: HostServices, capacity: usize) -> Self {
        Self { config, services, admission: Arc::new(Semaphore::new(capacity)) }
    }
}

pub(crate) fn router<S: Clone + Send + Sync + 'static>(state: ApiState) -> Router<S> {
    Router::new()
        .route(CAPABILITIES_PATH, get(capabilities))
        .route(
            "/api/v1/{*endpoint}",
            post(call_endpoint).layer(middleware::from_fn_with_state(state.clone(), guard)),
        )
        .layer(middleware::map_response(no_store))
        .with_state(state)
}

async fn capabilities() -> Response {
    Json(Capabilities::server()).into_response()
}

/// 在请求体被读取之前拒绝不合格的调用，未登录的客户端无法让服务端缓冲请求体。
async fn guard(State(state): State<ApiState>, request: Request, next: Next) -> Response {
    match reject_request(&state.config, request.headers()) {
        Some(rejection) => rejection,
        None => next.run(request).await,
    }
}

async fn call_endpoint(
    State(state): State<ApiState>,
    Path(endpoint): Path<String>,
    request: Request,
) -> Response {
    let read = axum::body::to_bytes(request.into_body(), MAX_API_BODY_BYTES);
    let body = match tokio::time::timeout(BODY_READ_TIMEOUT, read).await {
        Ok(Ok(body)) => body,
        Ok(Err(_)) => {
            return error_response(
                StatusCode::PAYLOAD_TOO_LARGE,
                ApiError::message("请求体读取失败或超过 16 MiB 上限"),
            );
        }
        Err(_) => {
            return error_response(
                StatusCode::REQUEST_TIMEOUT,
                ApiError::message("请求体读取超时"),
            );
        }
    };
    let Ok(permit) = state.admission.clone().try_acquire_owned() else {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            ApiError::message("服务端繁忙，请稍后重试"),
        );
    };

    let services = state.services.clone();
    let task_endpoint = endpoint.clone();
    let outcome = tokio::spawn(async move {
        let _permit = permit;
        dispatch(&services, &task_endpoint, &body).await
    })
    .await;

    match outcome {
        Ok(Ok(json)) => {
            ([(header::CONTENT_TYPE, HeaderValue::from_static("application/json"))], json)
                .into_response()
        }
        Ok(Err(DispatchError::UnknownEndpoint)) => {
            error_response(StatusCode::NOT_FOUND, ApiError::message("未知的接口"))
        }
        Ok(Err(DispatchError::BadRequest(message))) => error_response(
            StatusCode::BAD_REQUEST,
            ApiError::message(format!("请求参数无效：{message}")),
        ),
        Ok(Err(DispatchError::Failed(error))) => {
            tracing::debug!(endpoint = %endpoint, error = %format!("{error:#}"), "接口调用失败");
            error_response(StatusCode::UNPROCESSABLE_ENTITY, ApiError::from_anyhow(&error))
        }
        Err(join_error) => {
            tracing::error!(endpoint = %endpoint, error = %join_error, "接口任务异常退出");
            error_response(StatusCode::INTERNAL_SERVER_ERROR, ApiError::message("服务端内部错误"))
        }
    }
}

/// 不满足调用条件时返回拒绝响应。
fn reject_request(config: &AuthConfig, headers: &HeaderMap) -> Option<Response> {
    if !request_has_valid_session(config, headers) {
        return Some(error_response(
            StatusCode::UNAUTHORIZED,
            ApiError::message("未登录或会话已过期，请重新登录"),
        ));
    }
    // 恰好一个 Content-Type，且是 JSON；重复的头不按"第一个"放行。
    let is_json = single_header(headers, header::CONTENT_TYPE.as_str())
        .and_then(|value| value.split(';').next())
        .is_some_and(|mime| mime.trim().eq_ignore_ascii_case("application/json"));
    if !is_json {
        return Some(error_response(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            ApiError::message("接口只接受 application/json"),
        ));
    }
    let protocol_matches = single_header(headers, PROTOCOL_HEADER)
        .is_some_and(|value| value.trim() == PROTOCOL_VERSION.to_string());
    if !protocol_matches {
        return Some(error_response(
            StatusCode::CONFLICT,
            ApiError::message("页面与服务端版本不一致，请刷新页面"),
        ));
    }
    if !origin_matches_host(config, headers) {
        return Some(error_response(StatusCode::FORBIDDEN, ApiError::message("拒绝跨源请求")));
    }
    // 声明的长度超限时立即拒绝，不等请求体；没有声明长度的分块请求由读取时的上限兜底。
    let declared_too_large = single_header(headers, header::CONTENT_LENGTH.as_str())
        .and_then(|value| value.trim().parse::<u64>().ok())
        .is_some_and(|length| length > MAX_API_BODY_BYTES as u64);
    if declared_too_large {
        return Some(error_response(
            StatusCode::PAYLOAD_TOO_LARGE,
            ApiError::message("请求体超过 16 MiB 上限"),
        ));
    }
    None
}

/// 恰好出现一次且是可见 ASCII 的头部值；缺失、重复或含非法字节都返回 `None`。
fn single_header<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    let mut values = headers.get_all(name).iter();
    match (values.next(), values.next()) {
        (Some(value), None) => value.to_str().ok(),
        _ => None,
    }
}

/// 同源校验。没有 `Origin` 时放行（浏览器的跨源 POST 都会带）；带了就必须恰好一个、
/// 是规范的 HTTP(S) 源（不含路径），并与服务地址的 scheme、主机、端口一致。
///
/// 服务地址取自 `Host`；只有开启 `trust_proxy_headers` 时才改用 `X-Forwarded-Host` /
/// `X-Forwarded-Proto`。后端不知道外部 scheme 时（没有可信的转发头），`secure_cookie`
/// 开启即要求 https，否则接受 http 与 https。
fn origin_matches_host(config: &AuthConfig, headers: &HeaderMap) -> bool {
    if headers.get(header::ORIGIN).is_none() {
        return true;
    }
    let Some(raw_origin) = single_header(headers, header::ORIGIN.as_str()) else {
        return false;
    };
    let Ok(origin) = url::Url::parse(raw_origin) else {
        return false;
    };
    if !matches!(origin.scheme(), "http" | "https")
        || origin.origin().ascii_serialization() != raw_origin
    {
        return false;
    }
    let Some(origin_host) = origin.host_str() else {
        return false;
    };
    let origin_authority = match origin.port() {
        Some(port) => format!("{origin_host}:{port}"),
        None => origin_host.to_string(),
    };

    let forwarded = |name: &str| {
        config
            .trust_proxy_headers
            .then(|| single_header(headers, name))
            .flatten()
            .and_then(|value| value.split(',').next())
            .map(str::trim)
    };
    let expected_scheme =
        forwarded("x-forwarded-proto").or_else(|| config.secure_cookie.then_some("https"));
    if expected_scheme.is_some_and(|scheme| !scheme.eq_ignore_ascii_case(origin.scheme())) {
        return false;
    }
    let Some(request_authority) =
        forwarded("x-forwarded-host").or_else(|| single_header(headers, header::HOST.as_str()))
    else {
        return false;
    };
    let request_authority = request_authority.trim();
    request_authority.eq_ignore_ascii_case(&origin_authority)
        || default_port_elided(request_authority, origin.scheme())
            .is_some_and(|authority| authority.eq_ignore_ascii_case(&origin_authority))
}

/// `Host: example.com:443` 与 `Origin: https://example.com` 是同一个地址。
fn default_port_elided<'a>(authority: &'a str, scheme: &str) -> Option<&'a str> {
    let default_port = match scheme {
        "https" => ":443",
        "http" => ":80",
        _ => return None,
    };
    authority.strip_suffix(default_port)
}

fn error_response(status: StatusCode, error: ApiError) -> Response {
    (status, Json(error)).into_response()
}

/// `/api` 下的全部响应（含 405、413 等框架生成的）都不缓存。
async fn no_store(mut response: Response) -> Response {
    response.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}
#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::Router;
    use reqwest::StatusCode;
    use rssr_host::protocol::{ApiError, Capabilities, PROTOCOL_HEADER};
    use rssr_infra::outbound::OutboundPolicy;

    use super::{ApiState, router};
    use crate::{
        auth::test_support,
        library::{LibraryConfig, open_library},
    };

    struct TestServer {
        base: String,
        cookie: String,
        client: reqwest::Client,
        data_dir: std::path::PathBuf,
        server: tokio::task::JoinHandle<()>,
    }

    impl TestServer {
        async fn start() -> Self {
            Self::start_with_capacity(super::MAX_IN_FLIGHT_CALLS).await
        }

        async fn start_with_capacity(capacity: usize) -> Self {
            let data_dir = std::env::temp_dir().join(format!(
                "rssr-web-api-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("clock after epoch")
                    .as_nanos()
            ));
            let services = open_library(&LibraryConfig {
                data_dir: data_dir.clone(),
                outbound: OutboundPolicy::PublicOnly,
            })
            .await
            .expect("open library");
            let config = Arc::new(test_support::config());
            let cookie = test_support::session_cookie(&config);
            let app: Router = router(ApiState::with_capacity(config, services, capacity));
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let base = format!("http://{}", listener.local_addr().unwrap());
            let server = tokio::spawn(async move {
                axum::serve(listener, app).await.unwrap();
            });
            // The machine may have HTTP(S)_PROXY set; local test traffic must not go through it.
            let client = reqwest::Client::builder().no_proxy().build().unwrap();
            Self { base, cookie, client, data_dir, server }
        }

        /// 带会话 cookie，但不带内容类型与协议头。
        fn bare_post(&self, endpoint: &str) -> reqwest::RequestBuilder {
            self.client
                .post(format!("{}/api/v1/{endpoint}", self.base))
                .header(reqwest::header::COOKIE, &self.cookie)
        }

        fn post(&self, endpoint: &str) -> reqwest::RequestBuilder {
            self.bare_post(endpoint)
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .header(PROTOCOL_HEADER, "1")
        }

        fn stop(self) {
            self.server.abort();
            let _ = std::fs::remove_dir_all(&self.data_dir);
        }
    }

    #[test]
    fn origin_check_covers_default_ports_and_trusted_forwarded_host() {
        use axum::http::{HeaderMap, HeaderValue};

        fn headers(pairs: &[(&'static str, &'static str)]) -> HeaderMap {
            let mut map = HeaderMap::new();
            for (name, value) in pairs {
                map.append(*name, HeaderValue::from_static(value));
            }
            map
        }
        let direct = test_support::config();
        let mut behind_proxy = test_support::config();
        behind_proxy.trust_proxy_headers = true;

        assert!(super::origin_matches_host(&direct, &headers(&[("host", "example.com")])));
        assert!(super::origin_matches_host(
            &direct,
            &headers(&[("host", "example.com"), ("origin", "https://example.com")])
        ));
        assert!(super::origin_matches_host(
            &direct,
            &headers(&[("host", "example.com:443"), ("origin", "https://example.com")])
        ));
        assert!(!super::origin_matches_host(
            &direct,
            &headers(&[("host", "example.com:8443"), ("origin", "https://example.com")])
        ));
        assert!(!super::origin_matches_host(
            &direct,
            &headers(&[("host", "example.com"), ("origin", "null")])
        ));
        // A reverse proxy that rewrites Host: only a trusted X-Forwarded-Host can restore it.
        let proxied = [
            ("host", "127.0.0.1:8080"),
            ("x-forwarded-host", "example.com"),
            ("origin", "https://example.com"),
        ];
        assert!(!super::origin_matches_host(&direct, &headers(&proxied)));
        assert!(super::origin_matches_host(&behind_proxy, &headers(&proxied)));

        // Origins are serialized origins only: no path, exactly one header.
        assert!(!super::origin_matches_host(
            &direct,
            &headers(&[("host", "example.com"), ("origin", "https://example.com/path")])
        ));
        assert!(!super::origin_matches_host(
            &direct,
            &headers(&[
                ("host", "example.com"),
                ("origin", "https://example.com"),
                ("origin", "https://example.com"),
            ])
        ));
        // The scheme counts: Secure-cookie deployments are https, and a trusted proxy's
        // X-Forwarded-Proto overrides that.
        let mut secure = test_support::config();
        secure.secure_cookie = true;
        assert!(!super::origin_matches_host(
            &secure,
            &headers(&[("host", "example.com"), ("origin", "http://example.com")])
        ));
        assert!(super::origin_matches_host(
            &secure,
            &headers(&[("host", "example.com"), ("origin", "https://example.com")])
        ));
        assert!(!super::origin_matches_host(
            &behind_proxy,
            &headers(&[
                ("host", "127.0.0.1:8080"),
                ("x-forwarded-host", "example.com"),
                ("x-forwarded-proto", "http"),
                ("origin", "https://example.com"),
            ])
        ));
    }

    /// 只发请求头、不发请求体：守卫必须在读取请求体之前作答，否则这个请求会一直挂着。
    async fn status_line_without_body(base: &str, extra_headers: &str) -> String {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let address = base.trim_start_matches("http://");
        let mut socket = tokio::net::TcpStream::connect(address).await.unwrap();
        let request = format!(
            "POST /api/v1/settings/load HTTP/1.1\r\nHost: {address}\r\n{extra_headers}\r\n"
        );
        socket.write_all(request.as_bytes()).await.unwrap();
        let mut response = vec![0; 1024];
        let read =
            tokio::time::timeout(std::time::Duration::from_secs(5), socket.read(&mut response))
                .await
                .unwrap_or_else(|_| panic!("no answer before the body for: {extra_headers}"))
                .unwrap();
        String::from_utf8_lossy(&response[..read]).lines().next().unwrap_or_default().to_string()
    }

    #[tokio::test]
    async fn rejections_happen_before_the_body_is_read() {
        let server = TestServer::start().await;

        let anonymous = status_line_without_body(
            &server.base,
            "Content-Type: application/json\r\nx-rssr-protocol: 1\r\nContent-Length: 1000000\r\n",
        )
        .await;
        assert!(anonymous.contains("401"), "unexpected status line: {anonymous}");

        let oversized = status_line_without_body(
            &server.base,
            &format!(
                "Cookie: {}\r\nContent-Type: application/json\r\nx-rssr-protocol: 1\r\nContent-Length: {}\r\n",
                server.cookie,
                super::MAX_API_BODY_BYTES + 1
            ),
        )
        .await;
        assert!(oversized.contains("413"), "unexpected status line: {oversized}");

        server.stop();
    }

    #[tokio::test]
    async fn saturated_admission_returns_503_json() {
        let server = TestServer::start_with_capacity(0).await;
        let response = server.post("settings/load").body("null").send().await.unwrap();
        assert_eq!(response.headers().get(reqwest::header::CACHE_CONTROL).unwrap(), "no-store");
        let (status, error) = error_of(response).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert!(error.messages[0].contains("繁忙"));
        server.stop();
    }

    async fn error_of(response: reqwest::Response) -> (StatusCode, ApiError) {
        let status = response.status();
        (status, response.json().await.expect("error body is ApiError JSON"))
    }

    #[tokio::test]
    async fn capabilities_are_public_and_identify_server_mode() {
        let server = TestServer::start().await;
        let response =
            server.client.get(format!("{}/api/v1/capabilities", server.base)).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.json::<Capabilities>().await.unwrap(), Capabilities::server());
        server.stop();
    }

    #[tokio::test]
    async fn requests_without_session_json_protocol_or_same_origin_are_rejected() {
        let server = TestServer::start().await;
        let url = format!("{}/api/v1/settings/load", server.base);

        let (status, error) = error_of(
            server
                .client
                .post(&url)
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .header(PROTOCOL_HEADER, "1")
                .body("null")
                .send()
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert!(error.messages[0].contains("登录"));

        let response = server
            .bare_post("settings/load")
            .header(reqwest::header::CONTENT_TYPE, "text/plain")
            .header(PROTOCOL_HEADER, "1")
            .body("null")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);

        let response = server
            .bare_post("settings/load")
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .header(PROTOCOL_HEADER, "999")
            .body("null")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);

        let response = server
            .post("settings/load")
            .header(reqwest::header::ORIGIN, "http://evil.example")
            .body("null")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);

        let response = server
            .post("settings/load")
            .header(reqwest::header::ORIGIN, server.base.clone())
            .body("null")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "same-origin request must pass");

        server.stop();
    }

    #[tokio::test]
    async fn endpoints_return_typed_json_and_typed_errors() {
        let server = TestServer::start().await;

        let response = server.post("settings/load").body("null").send().await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers().get(reqwest::header::CACHE_CONTROL).unwrap(), "no-store");
        let settings: rssr_domain::UserSettings = response.json().await.unwrap();
        assert_eq!(settings, rssr_domain::UserSettings::default());

        let (status, _) =
            error_of(server.post("feeds/drop_database").body("null").send().await.unwrap()).await;
        assert_eq!(status, StatusCode::NOT_FOUND);

        let (status, _) =
            error_of(server.post("reader/load_entry").body("\"x\"").send().await.unwrap()).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);

        // PublicOnly: the server must refuse to probe a loopback feed on the user's behalf.
        let (status, error) = error_of(
            server
                .post("feeds/add_subscription")
                .body(r#"{"raw_url":"http://127.0.0.1:1/feed.xml","fallback_site_url":null}"#)
                .send()
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert!(
            error.messages.iter().any(|message| message.contains("内网或本地地址")),
            "unexpected error chain: {:?}",
            error.messages
        );

        server.stop();
    }
}
