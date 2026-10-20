//! 服务端模式的 `/api/v1`：把 [`rssr_host::protocol`] 的端点暴露给浏览器端。
//!
//! 每个调用都要通过：有效会话（否则 401，不重定向到登录页）、`Content-Type: application/json`、
//! 匹配的协议版本头，以及与服务地址一致的 `Origin`（若带）。自定义头与 JSON 类型让跨源请求
//! 必须先经预检，而本服务不返回任何 CORS 许可；`Origin` 校验是额外一层。
//!
//! 调用在独立任务里执行：浏览器断开或刷新页面不会中断进行中的刷新或写入。
use std::sync::Arc;

use axum::{
    Json, Router,
    body::Bytes,
    extract::{DefaultBodyLimit, Path, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use rssr_host::{
    HostServices,
    protocol::{ApiError, CAPABILITIES_PATH, Capabilities, PROTOCOL_HEADER, PROTOCOL_VERSION},
    server::{DispatchError, dispatch},
};

use crate::auth::{AuthConfig, request_has_valid_session};

/// 导入的配置包与 OPML 都是整段文本，放宽到 16 MiB；axum 默认只有 2 MiB。
const MAX_API_BODY_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone)]
pub(crate) struct ApiState {
    pub(crate) config: Arc<AuthConfig>,
    pub(crate) services: HostServices,
}

pub(crate) fn router<S: Clone + Send + Sync + 'static>(state: ApiState) -> Router<S> {
    Router::new()
        .route(CAPABILITIES_PATH, get(capabilities))
        .route("/api/v1/{*endpoint}", post(call_endpoint))
        .layer(DefaultBodyLimit::max(MAX_API_BODY_BYTES))
        .with_state(state)
}

async fn capabilities() -> Response {
    no_store(Json(Capabilities::server()).into_response())
}

async fn call_endpoint(
    State(state): State<ApiState>,
    Path(endpoint): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if let Some(rejection) = reject_request(&state.config, &headers) {
        return rejection;
    }

    let services = state.services.clone();
    let task_endpoint = endpoint.clone();
    let outcome =
        tokio::spawn(async move { dispatch(&services, &task_endpoint, &body).await }).await;

    match outcome {
        Ok(Ok(json)) => no_store(
            ([(header::CONTENT_TYPE, HeaderValue::from_static("application/json"))], json)
                .into_response(),
        ),
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
    let mut content_types = headers.get_all(header::CONTENT_TYPE).iter();
    let is_json = matches!((content_types.next(), content_types.next()), (Some(value), None)
        if value
            .to_str()
            .ok()
            .and_then(|value| value.split(';').next())
            .is_some_and(|mime| mime.trim().eq_ignore_ascii_case("application/json")));
    if !is_json {
        return Some(error_response(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            ApiError::message("接口只接受 application/json"),
        ));
    }
    let mut protocols = headers.get_all(PROTOCOL_HEADER).iter();
    let protocol_matches = matches!((protocols.next(), protocols.next()), (Some(value), None)
        if value.to_str().is_ok_and(|value| value.trim() == PROTOCOL_VERSION.to_string()));
    if !protocol_matches {
        return Some(error_response(
            StatusCode::CONFLICT,
            ApiError::message("页面与服务端版本不一致，请刷新页面"),
        ));
    }
    if !origin_matches_host(config, headers) {
        return Some(error_response(StatusCode::FORBIDDEN, ApiError::message("拒绝跨源请求")));
    }
    None
}

/// 没有 `Origin` 时放行（同源 GET 之外的浏览器请求都会带）；带了就必须与服务地址一致。
/// 只比较主机与端口：TLS 由反向代理终止时，后端看到的协议与浏览器不同。
fn origin_matches_host(config: &AuthConfig, headers: &HeaderMap) -> bool {
    let Some(origin) = headers.get(header::ORIGIN) else {
        return true;
    };
    let Some(origin) = origin.to_str().ok().and_then(|value| url::Url::parse(value).ok()) else {
        return false;
    };
    let Some(origin_host) = origin.host_str() else {
        return false;
    };
    let origin_authority = match origin.port() {
        Some(port) => format!("{origin_host}:{port}"),
        None => origin_host.to_string(),
    };

    let forwarded_host = config
        .trust_proxy_headers
        .then(|| headers.get("x-forwarded-host"))
        .flatten()
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next());
    let Some(request_authority) =
        forwarded_host.or_else(|| headers.get(header::HOST).and_then(|value| value.to_str().ok()))
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
    no_store((status, Json(error)).into_response())
}

fn no_store(mut response: Response) -> Response {
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
            let app: Router = router(ApiState { config, services });
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
