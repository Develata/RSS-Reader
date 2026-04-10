mod auth;
mod proxy;
mod smoke;

use std::{collections::HashMap, sync::Arc};

use anyhow::{Context, Result, ensure};
use axum::{
    Router,
    http::{HeaderValue, StatusCode, header},
    middleware,
    response::{Html, IntoResponse},
    routing::get,
};
use clap::Parser;
use tokio::fs;
use tokio::sync::Mutex;
use tower_http::services::{ServeDir, ServeFile};
use tracing::info;
use tracing_subscriber::EnvFilter;

use crate::{
    auth::{
        AppState, generate_password_hash, handle_login, handle_logout, load_config, require_auth,
        session_probe, show_login,
    },
    proxy::feed_proxy,
    smoke::{browser_feed_smoke, feed_fixture, smoke_helpers_enabled},
};

const FAVICON_ICO: &[u8] = include_bytes!("../../../icons/icon.ico");

#[derive(Parser, Debug)]
struct Cli {
    #[arg(long, value_name = "PASSWORD")]
    print_password_hash: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();
    let cli = Cli::parse();

    if let Some(password) = cli.print_password_hash {
        println!("{}", generate_password_hash(&password)?);
        return Ok(());
    }

    let config = Arc::new(load_config()?);
    let app_state =
        AppState { config: config.clone(), login_throttle: Arc::new(Mutex::new(HashMap::new())) };
    ensure!(
        config.static_dir.join("index.html").exists(),
        "静态资源目录缺少 index.html：{}",
        config.static_dir.display()
    );

    let smoke_helpers = if smoke_helpers_enabled() {
        Router::new()
            .route("/__codex/browser-feed-smoke", get(browser_feed_smoke))
            .route("/__codex/feed-fixture.xml", get(feed_fixture))
            .with_state(app_state.clone())
    } else {
        Router::new()
    };

    let protected = Router::new()
        .route("/", get(serve_app_shell))
        .route("/entries", get(serve_app_shell))
        .route("/entries/{entry_id}", get(serve_app_shell))
        .route("/feeds", get(serve_app_shell))
        .route("/feeds/{feed_id}/entries", get(serve_app_shell))
        .route("/settings", get(serve_app_shell))
        .route("/session-probe", get(session_probe))
        .route("/feed-proxy", get(feed_proxy))
        .fallback_service(
            ServeDir::new(config.static_dir.clone())
                .not_found_service(ServeFile::new(config.static_dir.join("index.html"))),
        )
        .layer(middleware::from_fn_with_state(app_state.clone(), require_auth));

    let app = Router::new()
        .route("/login", get(show_login).post(handle_login))
        .route("/logout", get(handle_logout))
        .route("/healthz", get(healthz))
        .route("/favicon.ico", get(favicon))
        .merge(smoke_helpers)
        .merge(protected)
        .with_state(app_state);

    info!("starting rssr-web on {}", config.bind_addr);
    let listener = tokio::net::TcpListener::bind(config.bind_addr)
        .await
        .with_context(|| format!("绑定监听地址失败：{}", config.bind_addr))?;

    axum::serve(listener, app).await.context("启动 Web 服务失败")?;

    Ok(())
}

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();
}

async fn healthz() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

async fn serve_app_shell(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> impl IntoResponse {
    let index_path = state.config.static_dir.join("index.html");
    match fs::read_to_string(&index_path).await {
        Ok(index_html) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, HeaderValue::from_static("text/html; charset=utf-8"))],
            Html(index_html),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("读取前端入口失败：{} ({err})", index_path.display()),
        )
            .into_response(),
    }
}

async fn favicon() -> impl IntoResponse {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, HeaderValue::from_static("image/x-icon"))],
        FAVICON_ICO,
    )
}
