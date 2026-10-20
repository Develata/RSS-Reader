mod api;
#[cfg(test)]
mod api_tests;
mod auth;
#[cfg(test)]
mod client_tests;
#[cfg(test)]
mod http_tests;
mod jobs;

use std::{
    io::{IsTerminal, Write},
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, ensure};
use axum::{
    Router,
    http::{HeaderValue, StatusCode, header},
    response::{Html, IntoResponse},
    routing::get,
};
use clap::{Parser, Subcommand};
use rssr_infra::{
    local_backend::native_local_backend_with,
    outbound::ensure_server_policy,
    store::{StoreOptions, local::open_library_in},
};
use tokio::fs;
use tower_http::services::{ServeDir, ServeFile};
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

use crate::{
    api::ApiState,
    auth::{
        AppState, AuthStore, generate_password_hash, load_config, make_library_private, open_store,
        prepare_data_dir, with_auth,
    },
};

const FAVICON_ICO: &[u8] = include_bytes!("../../../icons/icon.ico");
/// 停止的时间预算。Docker 在 SIGTERM 之后 10 秒发 SIGKILL，三段加起来要留出余量：
/// 等待进行中的请求、结束本次运行（[`AuthStore::end_run`]），以及关闭运行时。
const SHUTDOWN_GRACE: Duration = Duration::from_secs(4);
const END_RUN_BUDGET: Duration = Duration::from_secs(3);
const RUNTIME_SHUTDOWN: Duration = Duration::from_secs(1);

/// RSS-Reader 的 Web 服务：登录保护、阅读器接口与静态包托管。
#[derive(Parser)]
struct Cli {
    /// 数据目录：资料库 library.sqlite3、出站策略 outbound.json 与认证库 auth.sqlite3 在这里。
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

fn main() -> Result<()> {
    // Files this process creates are its user's alone (the library, its WAL and shared memory,
    // the policy): a data directory may be readable by others (review C3-R1 F02).
    #[cfg(unix)]
    // SAFETY: umask has no preconditions and cannot fail; set before any thread starts.
    unsafe {
        libc::umask(0o077);
    }
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
        None => {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .context("无法启动异步运行时")?;
            let result = runtime.block_on(serve(&data_dir));
            // 不等仍在跑的阻塞任务：认证库已经关闭（`end_run`），它们不会再改动会话。
            runtime.shutdown_timeout(RUNTIME_SHUTDOWN);
            result
        }
    }
}

async fn serve(data_dir: &Path) -> Result<()> {
    let config = Arc::new(load_config()?);
    ensure!(
        config.static_dir.join("index.html").exists(),
        "静态资源目录缺少 index.html：{}",
        config.static_dir.display()
    );
    // 先绑定再开始运行：端口被占时直接退出，不留下运行标记。
    let listener = tokio::net::TcpListener::bind(config.bind_addr)
        .await
        .with_context(|| format!("绑定监听地址失败：{}", config.bind_addr))?;
    let local = listener.local_addr().context("读取监听地址失败")?;
    // 资料库在认证库开始运行之前打开：打不开时直接退出，不留下运行标记（否则下次启动会因为
    // 一个与会话无关的故障吊销全部会话）。
    let backend = Arc::new(open_backend(data_dir).await?);
    let store = Arc::new(open_store(data_dir)?);
    // 取得服务器排他锁（`open_store` 的 `begin_run`）之后才开始定时刷新：同一目录上被拒绝的
    // 第二个服务器不抓取任何 feed（审计 C3-R1 F12）。
    rssr_api::Backend::ensure_auto_refresh_started(backend.as_ref());
    let api = ApiState::new(backend);
    let state = AppState::new(config, Arc::clone(&store), local)?;

    info!(build = api::BUILD_ID, "starting rssr-web on {local}");
    let stopping = Arc::new(tokio::sync::Notify::new());
    // 登录限速在不信任代理头时按 TCP 对端地址区分客户端，需要连接信息（审计 F1）。
    let server =
        axum::serve(listener, app(state, api).into_make_service_with_connect_info::<SocketAddr>())
            .with_graceful_shutdown({
                let stopping = Arc::clone(&stopping);
                async move {
                    shutdown_signal().await;
                    stopping.notify_one();
                }
            });
    tokio::select! {
        result = std::future::IntoFuture::into_future(server) => result.context("启动 Web 服务失败")?,
        () = async {
            stopping.notified().await;
            tokio::time::sleep(SHUTDOWN_GRACE).await;
        } => warn!("停止时放弃了 {SHUTDOWN_GRACE:?} 内没有处理完的请求"),
    }

    // 正常停止：关闭认证库；没有删不掉的失效会话时移除运行标记，下次启动保留全部会话。
    // 收尾在独立线程上做（阻塞线程池可能排满），这里最多等到时限：超时也不取消它，但它在
    // 时限之后不会再移除标记（`end_run`）。日志写入本身阻塞不在这个预算内。
    let deadline = Instant::now() + END_RUN_BUDGET;
    let (finished, outcome) = tokio::sync::oneshot::channel();
    std::thread::Builder::new()
        .name("rssr-web-end-run".into())
        .spawn(move || {
            let _ = finished.send(store.end_run(deadline));
        })
        .context("无法启动收尾线程")?;
    match tokio::time::timeout_at(tokio::time::Instant::from_std(deadline), outcome).await {
        Ok(Ok(Ok(true))) => info!("rssr-web 已停止"),
        Ok(Ok(Ok(false))) => {
            warn!("没能确认失效会话都已删除：保留运行标记，下次启动时吊销全部会话");
        }
        Ok(Ok(Err(error))) => warn!(%error, "结束运行失败：保留运行标记"),
        Ok(Err(_)) | Err(_) => warn!("停止的收尾超过时限：保留运行标记，下次启动时吊销全部会话"),
    }
    Ok(())
}

/// 数据目录里的资料库与出站策略（没有时写入默认的服务器策略），以及其上的后端（定时刷新由
/// 调用方在取得服务器排他锁之后开始）。数据目录不存在时先以仅所有者可访问的权限创建；已有的
/// 资料库文件改为仅所有者可读写。首次打开时导入同一目录里的 v0.1.x 资料库。
async fn open_backend(
    data_dir: &Path,
) -> Result<rssr_infra::local_backend::LocalBackend<rssr_infra::local_backend::NativeHost>> {
    prepare_data_dir(data_dir)?;
    let policy = ensure_server_policy(data_dir).context("准备出站策略失败")?;
    make_library_private(data_dir)?;
    let opened =
        open_library_in(data_dir, StoreOptions::default()).await.context("打开资料库失败")?;
    if let Some(imported) = &opened.imported {
        info!(?imported, "已从 v0.1 数据导入资料库");
    }
    // Again for the files the open itself created (the umask of `main` covers them too).
    make_library_private(data_dir)?;
    info!(library = %opened.path.display(), "已打开资料库");
    native_local_backend_with(opened.store, policy).context("建立后端失败")
}

/// Ctrl-C，或 Unix 上的 SIGTERM（`docker stop`、systemd）。注册失败的那一路不会触发停止。
async fn shutdown_signal() {
    let interrupt = async {
        if tokio::signal::ctrl_c().await.is_err() {
            std::future::pending::<()>().await;
        }
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        () = interrupt => {}
        () = terminate => {}
    }
}

/// 全部路由：应用页面、阅读器接口（[`api`]）与静态包要求登录（[`with_auth`]）。服务器自己抓
/// feed，不再为浏览器代抓：`/feed-proxy` 与其他未知路径一样，登录后得到 404（C4）。
fn app(state: AppState, api: ApiState) -> Router {
    let static_dir = state.config.static_dir.clone();
    let protected = Router::new()
        .route("/", get(serve_app_shell))
        .route("/entries", get(serve_app_shell))
        .route("/entries/{entry_id}", get(serve_app_shell))
        .route("/feeds", get(serve_app_shell))
        .route("/feeds/{feed_id}/entries", get(serve_app_shell))
        .route("/settings", get(serve_app_shell))
        .fallback_service(
            ServeDir::new(&static_dir)
                .not_found_service(ServeFile::new(static_dir.join("index.html"))),
        );
    with_auth(protected, api::routes(api), state)
        .route("/healthz", get(healthz))
        .route("/favicon.ico", get(favicon))
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
