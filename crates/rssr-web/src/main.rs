mod auth;
#[cfg(test)]
mod http_tests;
mod proxy;

use std::{
    io::{IsTerminal, Write},
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, Result, ensure};
use axum::{
    Router,
    http::{HeaderValue, StatusCode, header},
    response::{Html, IntoResponse},
    routing::get,
};
use clap::{Parser, Subcommand};
use tokio::fs;
use tower_http::services::{ServeDir, ServeFile};
use tracing::info;
use tracing_subscriber::EnvFilter;

use crate::{
    auth::{
        AppState, AuthStore, generate_password_hash, load_config, open_store, session_probe,
        with_auth,
    },
    proxy::feed_proxy,
};

const FAVICON_ICO: &[u8] = include_bytes!("../../../icons/icon.ico");

/// RSS-Reader 的 Web 服务：登录保护、静态包托管与 feed 代抓。
#[derive(Parser)]
struct Cli {
    /// 数据目录：认证库 auth.sqlite3 在这里。
    #[arg(long, env = "RSSR_DATA_DIR", global = true, value_name = "DIR")]
    data_dir: Option<PathBuf>,
    /// 打印密码的 Argon2 哈希（可用作 RSS_READER_WEB_PASSWORD_HASH）后退出。
    #[arg(long, value_name = "PASSWORD")]
    print_password_hash: Option<String>,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// 从标准输入读入新密码（一行），改密码并吊销全部会话。
    Passwd,
    /// 管理会话。
    Sessions {
        #[command(subcommand)]
        action: SessionsCommand,
    },
}

#[derive(Subcommand)]
enum SessionsCommand {
    /// 吊销全部会话，保留凭据（例如恢复备份之后）。
    RevokeAll,
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();
    let cli = Cli::parse();

    if let Some(password) = cli.print_password_hash {
        println!("{}", generate_password_hash(&password)?);
        return Ok(());
    }
    let data_dir = cli.data_dir.context("缺少数据目录：请用 --data-dir 或 RSSR_DATA_DIR 指定")?;
    match cli.command {
        Some(Command::Passwd) => passwd(&data_dir),
        Some(Command::Sessions { action: SessionsCommand::RevokeAll }) => revoke_all(&data_dir),
        None => serve(&data_dir).await,
    }
}

async fn serve(data_dir: &Path) -> Result<()> {
    let config = Arc::new(load_config()?);
    ensure!(
        config.static_dir.join("index.html").exists(),
        "静态资源目录缺少 index.html：{}",
        config.static_dir.display()
    );
    let store = Arc::new(open_store(data_dir)?);
    let listener = tokio::net::TcpListener::bind(config.bind_addr)
        .await
        .with_context(|| format!("绑定监听地址失败：{}", config.bind_addr))?;
    let local = listener.local_addr().context("读取监听地址失败")?;
    let state = AppState::new(config, store, local)?;

    info!("starting rssr-web on {local}");
    // 登录限速在不信任代理头时按 TCP 对端地址区分客户端，需要连接信息（审计 F1）。
    axum::serve(listener, app(state).into_make_service_with_connect_info::<SocketAddr>())
        .await
        .context("启动 Web 服务失败")?;

    Ok(())
}

/// 全部路由：应用页面、静态包与 `/feed-proxy` 要求登录（[`with_auth`]）。
fn app(state: AppState) -> Router {
    let static_dir = state.config.static_dir.clone();
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
            ServeDir::new(&static_dir)
                .not_found_service(ServeFile::new(static_dir.join("index.html"))),
        );
    with_auth(protected, state).route("/healthz", get(healthz)).route("/favicon.ico", get(favicon))
}

/// `rssr-web passwd`：改密码，凭据代数加一，吊销全部会话。认证库不存在时以新密码建立它。
fn passwd(data_dir: &Path) -> Result<()> {
    let stdin = std::io::stdin();
    if stdin.is_terminal() {
        eprint!("新密码（至少 8 个字符，会显示在屏幕上）：");
        std::io::stderr().flush()?;
    }
    let mut line = String::new();
    stdin.read_line(&mut line).context("读取新密码失败")?;
    let password = line.strip_suffix('\n').unwrap_or(&line);
    let password = password.strip_suffix('\r').unwrap_or(password);
    let hash = generate_password_hash(password)?;
    AuthStore::open(data_dir)?.change_password(&hash)?;
    eprintln!("密码已更新，全部会话已吊销。");
    Ok(())
}

/// `rssr-web sessions revoke-all`：吊销全部会话，保留凭据。
fn revoke_all(data_dir: &Path) -> Result<()> {
    let revoked = AuthStore::open(data_dir)?.revoke_all()?;
    eprintln!("已吊销 {revoked} 个会话。");
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
