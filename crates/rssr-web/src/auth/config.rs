use std::{
    env, fs,
    io::Write,
    net::SocketAddr,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow, ensure};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier, password_hash::SaltString};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use time::Duration;
use tracing::{info, warn};

const DEFAULT_LOGIN_RATE_LIMIT_WINDOW_MINUTES: i64 = 15;
const DEFAULT_LOGIN_RATE_LIMIT_BLOCK_MINUTES: i64 = 15;
const DEFAULT_LOGIN_RATE_LIMIT_MAX_FAILURES: u32 = 5;
const DEFAULT_AUTH_STATE_FILE_NAME: &str = ".rssr-web-auth.json";

#[derive(Clone)]
pub(crate) struct AuthConfig {
    pub(crate) bind_addr: SocketAddr,
    pub(crate) static_dir: PathBuf,
    pub(crate) username: String,
    pub(crate) password_policy: PasswordPolicy,
    pub(crate) session_secret: Vec<u8>,
    pub(crate) secure_cookie: bool,
    pub(crate) trust_proxy_headers: bool,
    pub(crate) session_ttl: Duration,
    pub(crate) login_rate_limit: LoginRateLimit,
}

#[derive(Clone)]
pub(crate) enum PasswordPolicy {
    Argon2Hash(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WebEnvironment {
    Development,
    Production,
}

#[derive(Clone)]
pub(crate) struct LoginRateLimit {
    pub(crate) max_failures: u32,
    pub(crate) window: Duration,
    pub(crate) block_for: Duration,
}

#[derive(Serialize, Deserialize)]
struct PersistedAuthState {
    password_hash: String,
    #[serde(default)]
    session_secret: Option<String>,
}

pub(crate) fn load_config() -> Result<AuthConfig> {
    let bind_addr = env::var("RSS_READER_WEB_BIND")
        .unwrap_or_else(|_| "0.0.0.0:80".to_string())
        .parse()
        .context("解析 RSS_READER_WEB_BIND 失败")?;
    let static_dir = PathBuf::from(
        env::var("RSS_READER_WEB_STATIC_DIR").unwrap_or_else(|_| "/app/public".to_string()),
    );
    let auth_state_file = resolve_auth_state_file();
    let username = required_env("RSS_READER_WEB_USERNAME")?;
    let password_policy = load_password_policy(&username, &auth_state_file)?;
    let session_secret = resolve_session_secret(&auth_state_file)?;
    ensure!(session_secret.len() >= 32, "RSS_READER_WEB_SESSION_SECRET 至少需要 32 个字符");
    let environment = env::var("RSS_READER_WEB_ENV")
        .ok()
        .map(parse_environment)
        .transpose()?
        .unwrap_or(WebEnvironment::Development);
    let secure_cookie = env::var("RSS_READER_WEB_SECURE_COOKIE")
        .ok()
        .map(|raw| matches!(raw.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false);
    let trust_proxy_headers = env::var("RSS_READER_WEB_TRUST_PROXY_HEADERS")
        .ok()
        .map(|raw| matches!(raw.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false);
    let session_ttl_hours = env::var("RSS_READER_WEB_SESSION_TTL_HOURS")
        .ok()
        .map(|raw| raw.parse::<i64>())
        .transpose()
        .context("解析 RSS_READER_WEB_SESSION_TTL_HOURS 失败")?
        .unwrap_or(12);
    ensure!(session_ttl_hours > 0, "会话时长必须大于 0");
    let login_rate_limit = load_login_rate_limit()?;

    if environment == WebEnvironment::Production {
        ensure!(secure_cookie, "生产环境必须开启 RSS_READER_WEB_SECURE_COOKIE=true。");
        ensure!(
            matches!(password_policy, PasswordPolicy::Argon2Hash(_)),
            "生产环境必须使用 Argon2 密码哈希（可来自 RSS_READER_WEB_PASSWORD_HASH、首次启动自动生成，或认证状态文件）。"
        );
    } else if env::var("RSS_READER_WEB_PASSWORD").is_ok()
        && env::var("RSS_READER_WEB_PASSWORD_HASH").is_err()
    {
        warn!(
            auth_state_file = %auth_state_file.display(),
            "rssr-web 正在使用明文 RSS_READER_WEB_PASSWORD 启动，并会自动生成 Argon2 哈希写入认证状态文件；建议后续移除明文密码配置。"
        );
    }

    Ok(AuthConfig {
        bind_addr,
        static_dir,
        username,
        password_policy,
        session_secret: session_secret.into_bytes(),
        secure_cookie,
        trust_proxy_headers,
        session_ttl: Duration::hours(session_ttl_hours),
        login_rate_limit,
    })
}

pub(crate) fn generate_password_hash(password: &str) -> Result<String> {
    ensure!(password.len() >= 8, "密码至少需要 8 个字符");
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|_| anyhow!("生成密码哈希失败"))?;
    Ok(hash.to_string())
}

pub(crate) fn verify_credentials(config: &AuthConfig, username: &str, password: &str) -> bool {
    if username.trim() != config.username {
        return false;
    }

    match &config.password_policy {
        PasswordPolicy::Argon2Hash(hash) => verify_password_hash(hash, password),
    }
}

pub(crate) fn verify_password_hash(hash: &str, password: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(hash) else {
        return false;
    };
    Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok()
}

fn required_env(name: &str) -> Result<String> {
    env::var(name).map_err(|_| anyhow!("缺少环境变量：{name}"))
}

fn optional_env(name: &str) -> Option<String> {
    env::var(name).ok().and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

fn resolve_auth_state_file() -> PathBuf {
    env::var("RSS_READER_WEB_AUTH_STATE_FILE").map(PathBuf::from).unwrap_or_else(|_| {
        env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(DEFAULT_AUTH_STATE_FILE_NAME)
    })
}

fn parse_environment(raw: String) -> Result<WebEnvironment> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "dev" | "development" | "local" => Ok(WebEnvironment::Development),
        "prod" | "production" => Ok(WebEnvironment::Production),
        _ => Err(anyhow!("RSS_READER_WEB_ENV 只能是 development 或 production")),
    }
}

fn load_password_policy(username: &str, auth_state_file: &Path) -> Result<PasswordPolicy> {
    resolve_password_policy(
        username,
        optional_env("RSS_READER_WEB_PASSWORD_HASH"),
        optional_env("RSS_READER_WEB_PASSWORD"),
        auth_state_file,
    )
}

fn resolve_session_secret(auth_state_file: &Path) -> Result<String> {
    if let Some(session_secret) = optional_env("RSS_READER_WEB_SESSION_SECRET") {
        ensure!(session_secret.len() >= 32, "RSS_READER_WEB_SESSION_SECRET 至少需要 32 个字符");
        persist_auth_state(auth_state_file, None, Some(&session_secret))?;
        return Ok(session_secret);
    }

    if let Some(state) = load_persisted_auth_state(auth_state_file)?
        && let Some(session_secret) = state.session_secret
    {
        ensure!(session_secret.len() >= 32, "认证状态文件中的 session secret 长度不足 32 个字符");
        return Ok(session_secret);
    }

    let generated = generate_session_secret();
    persist_auth_state(auth_state_file, None, Some(&generated))?;
    info!(
        auth_state_file = %auth_state_file.display(),
        "已自动生成并持久化 RSS_READER_WEB_SESSION_SECRET"
    );
    Ok(generated)
}

fn resolve_password_policy(
    username: &str,
    password_hash: Option<String>,
    password: Option<String>,
    auth_state_file: &Path,
) -> Result<PasswordPolicy> {
    if let Some(hash) = password_hash {
        validate_password_hash(&hash)?;
        persist_auth_state(auth_state_file, Some(&hash), None)?;
        return Ok(PasswordPolicy::Argon2Hash(hash));
    }

    if let Some(password) = password {
        ensure!(password.len() >= 8, "RSS_READER_WEB_PASSWORD 至少需要 8 个字符");

        if let Some(persisted_hash) = load_persisted_password_hash(auth_state_file)?
            && verify_password_hash(&persisted_hash, &password)
        {
            return Ok(PasswordPolicy::Argon2Hash(persisted_hash));
        }

        let generated_hash = generate_password_hash(&password)?;
        persist_auth_state(auth_state_file, Some(&generated_hash), None)?;
        info!(
            username = username,
            auth_state_file = %auth_state_file.display(),
            "已根据 RSS_READER_WEB_PASSWORD 自动生成并持久化密码哈希"
        );
        return Ok(PasswordPolicy::Argon2Hash(generated_hash));
    }

    if let Some(persisted_hash) = load_persisted_password_hash(auth_state_file)? {
        return Ok(PasswordPolicy::Argon2Hash(persisted_hash));
    }

    Err(anyhow!(
        "缺少环境变量：RSS_READER_WEB_PASSWORD_HASH、RSS_READER_WEB_PASSWORD，且认证状态文件中也没有可用的密码哈希"
    ))
}

fn load_persisted_password_hash(auth_state_file: &Path) -> Result<Option<String>> {
    let Some(state) = load_persisted_auth_state(auth_state_file)? else {
        return Ok(None);
    };
    validate_password_hash(&state.password_hash)?;
    Ok(Some(state.password_hash))
}

fn load_persisted_auth_state(auth_state_file: &Path) -> Result<Option<PersistedAuthState>> {
    if !auth_state_file.exists() {
        return Ok(None);
    }

    enforce_auth_state_file_permissions(auth_state_file)?;

    let raw = fs::read_to_string(auth_state_file)
        .with_context(|| format!("读取认证状态文件失败：{}", auth_state_file.display()))?;
    let state: PersistedAuthState = serde_json::from_str(&raw)
        .with_context(|| format!("解析认证状态文件失败：{}", auth_state_file.display()))?;
    Ok(Some(state))
}

fn persist_auth_state(
    auth_state_file: &Path,
    password_hash: Option<&str>,
    session_secret: Option<&str>,
) -> Result<()> {
    if let Some(password_hash) = password_hash {
        validate_password_hash(password_hash)?;
    }
    if let Some(session_secret) = session_secret {
        ensure!(session_secret.len() >= 32, "RSS_READER_WEB_SESSION_SECRET 至少需要 32 个字符");
    }

    if let Some(parent) = auth_state_file.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("创建认证状态目录失败：{}", parent.display()))?;
    }

    let existing = load_persisted_auth_state(auth_state_file)?;
    let password_hash = password_hash
        .map(ToOwned::to_owned)
        .or_else(|| existing.as_ref().map(|state| state.password_hash.clone()))
        .ok_or_else(|| anyhow!("写入认证状态文件失败：缺少密码哈希"))?;
    let session_secret = session_secret
        .map(ToOwned::to_owned)
        .or_else(|| existing.and_then(|state| state.session_secret));

    let payload =
        serde_json::to_string_pretty(&PersistedAuthState { password_hash, session_secret })
            .context("序列化认证状态文件失败")?;
    write_auth_state_file(auth_state_file, &payload)?;
    Ok(())
}

fn enforce_auth_state_file_permissions(auth_state_file: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        fs::set_permissions(auth_state_file, fs::Permissions::from_mode(0o600))
            .with_context(|| format!("收紧认证状态文件权限失败：{}", auth_state_file.display()))?;
    }

    Ok(())
}

fn write_auth_state_file(auth_state_file: &Path, payload: &str) -> Result<()> {
    #[cfg(unix)]
    {
        use std::fs::OpenOptions;
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .mode(0o600)
            .open(auth_state_file)
            .with_context(|| format!("写入认证状态文件失败：{}", auth_state_file.display()))?;
        file.write_all(payload.as_bytes())
            .with_context(|| format!("写入认证状态文件失败：{}", auth_state_file.display()))?;
        file.sync_all()
            .with_context(|| format!("同步认证状态文件失败：{}", auth_state_file.display()))?;
        fs::set_permissions(auth_state_file, fs::Permissions::from_mode(0o600))
            .with_context(|| format!("收紧认证状态文件权限失败：{}", auth_state_file.display()))?;
        Ok(())
    }

    #[cfg(not(unix))]
    {
        fs::write(auth_state_file, payload)
            .with_context(|| format!("写入认证状态文件失败：{}", auth_state_file.display()))?;
        Ok(())
    }
}

fn load_login_rate_limit() -> Result<LoginRateLimit> {
    let max_failures = env::var("RSS_READER_WEB_LOGIN_MAX_FAILURES")
        .ok()
        .map(|raw| raw.parse::<u32>())
        .transpose()
        .context("解析 RSS_READER_WEB_LOGIN_MAX_FAILURES 失败")?
        .unwrap_or(DEFAULT_LOGIN_RATE_LIMIT_MAX_FAILURES);
    ensure!(max_failures > 0, "RSS_READER_WEB_LOGIN_MAX_FAILURES 必须大于 0");

    let window_minutes = env::var("RSS_READER_WEB_LOGIN_WINDOW_MINUTES")
        .ok()
        .map(|raw| raw.parse::<i64>())
        .transpose()
        .context("解析 RSS_READER_WEB_LOGIN_WINDOW_MINUTES 失败")?
        .unwrap_or(DEFAULT_LOGIN_RATE_LIMIT_WINDOW_MINUTES);
    ensure!(window_minutes > 0, "RSS_READER_WEB_LOGIN_WINDOW_MINUTES 必须大于 0");

    let block_minutes = env::var("RSS_READER_WEB_LOGIN_BLOCK_MINUTES")
        .ok()
        .map(|raw| raw.parse::<i64>())
        .transpose()
        .context("解析 RSS_READER_WEB_LOGIN_BLOCK_MINUTES 失败")?
        .unwrap_or(DEFAULT_LOGIN_RATE_LIMIT_BLOCK_MINUTES);
    ensure!(block_minutes > 0, "RSS_READER_WEB_LOGIN_BLOCK_MINUTES 必须大于 0");

    Ok(LoginRateLimit {
        max_failures,
        window: Duration::minutes(window_minutes),
        block_for: Duration::minutes(block_minutes),
    })
}

fn validate_password_hash(raw: &str) -> Result<()> {
    PasswordHash::new(raw)
        .map_err(|_| anyhow!("RSS_READER_WEB_PASSWORD_HASH 不是合法的 Argon2 哈希"))?;
    Ok(())
}

fn generate_session_secret() -> String {
    let mut bytes = [0_u8; 32];
    OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}
