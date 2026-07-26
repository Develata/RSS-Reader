#[cfg(target_arch = "wasm32")]
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
#[cfg(target_arch = "wasm32")]
use sha2::{Digest, Sha256};

#[cfg(target_arch = "wasm32")]
#[path = "web_auth_browser.rs"]
mod browser;

#[cfg(target_arch = "wasm32")]
const AUTH_CONFIG_KEY: &str = "rssr-web-auth-config-v1";
#[cfg(target_arch = "wasm32")]
const AUTH_SESSION_KEY: &str = "rssr-web-auth-session-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebAuthState {
    Authenticated,
    PendingServerProbe,
    NeedsSetup,
    NeedsLogin,
}

#[cfg(target_arch = "wasm32")]
#[derive(Debug, Clone)]
pub struct StoredCredentials {
    pub username: String,
    password_hash: String,
    salt: String,
}

#[cfg(target_arch = "wasm32")]
impl StoredCredentials {
    fn new(username: String, password: &str) -> Self {
        let salt = generate_salt(&username);
        let password_hash = hash_password(&username, password, &salt);
        Self { username, password_hash, salt }
    }

    fn verify(&self, password: &str) -> bool {
        self.password_hash == hash_password(&self.username, password, &self.salt)
    }

    fn session_token(&self) -> String {
        let payload = format!("{}:{}", self.username, self.password_hash);
        URL_SAFE_NO_PAD.encode(Sha256::digest(payload.as_bytes()))
    }

    fn encode(&self) -> String {
        format!("{}\n{}\n{}", self.username, self.password_hash, self.salt)
    }

    fn decode(raw: &str) -> Option<Self> {
        let mut lines = raw.lines();
        let username = lines.next()?.trim().to_string();
        let password_hash = lines.next()?.trim().to_string();
        let salt = lines.next()?.trim().to_string();
        if username.is_empty() || password_hash.is_empty() || salt.is_empty() {
            return None;
        }
        Some(Self { username, password_hash, salt })
    }
}

#[cfg(target_arch = "wasm32")]
pub fn auth_state() -> WebAuthState {
    if browser::server_gate_present() {
        return WebAuthState::PendingServerProbe;
    }

    if !browser::local_web_auth_enabled() {
        return WebAuthState::Authenticated;
    }

    local_auth_state()
}

/// 服务端会话探测的结果。
///
/// 必须区分「会话过期」与「连不上」：此前两者都被压成 `false`，随后一律回落到
/// `local_auth_state()`，于是正式部署上会话一过期，用户就被引导去创建一组与服务端登录
/// 毫无关系的本地凭据。
/// 原生端只会产出 `Absent`（没有服务端门禁这个概念），其余分支仅在 wasm 上构造。
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerGateProbe {
    /// 没有服务端门禁，走本地判定。
    Absent,
    /// 会话有效。
    Authenticated,
    /// 有门禁但会话无效（`/session-probe` 在 `require_auth` 之后，过期时不会回 204）。
    SessionExpired,
    /// 探测本身失败（网络抖动、离线等），无法判定。
    Unreachable,
}

#[cfg(target_arch = "wasm32")]
pub async fn probe_server_gate() -> ServerGateProbe {
    if !browser::server_gate_present() {
        return ServerGateProbe::Absent;
    }

    // 拿不到 origin 说明连 window 都没有，不是网络故障；此时没有可探测的服务端门禁。
    let Some(origin) = browser::browser_origin() else {
        return ServerGateProbe::Absent;
    };
    let probe_url = format!("{origin}/session-probe");

    match reqwest::Client::new().get(probe_url).send().await {
        Ok(response) if response.status() == reqwest::StatusCode::NO_CONTENT => {
            ServerGateProbe::Authenticated
        }
        // 5xx 是服务端/反向代理故障，不是会话过期。若按过期处理去跳 /login，那个地址同样
        // 打不开——等于把「网络错误就整页跳转」这个错误换个入口重现一遍。
        Ok(response) if response.status().is_server_error() => {
            tracing::warn!(status = response.status().as_u16(), "服务端登录状态探测返回服务端错误");
            ServerGateProbe::Unreachable
        }
        Ok(_) => ServerGateProbe::SessionExpired,
        Err(error) => {
            tracing::warn!(error = %error, "确认服务端登录状态失败");
            ServerGateProbe::Unreachable
        }
    }
}

/// 服务端会话过期时的恢复动作：整页跳到服务端登录页。
#[cfg(target_arch = "wasm32")]
pub fn recover_from_expired_server_session() {
    browser::redirect_to_server_login();
}

#[cfg(target_arch = "wasm32")]
pub fn local_auth_state() -> WebAuthState {
    let Some(credentials) = load_credentials() else {
        return WebAuthState::NeedsSetup;
    };

    if browser::session_storage_get(AUTH_SESSION_KEY).as_deref()
        == Some(credentials.session_token().as_str())
    {
        WebAuthState::Authenticated
    } else {
        WebAuthState::NeedsLogin
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn probe_server_gate() -> ServerGateProbe {
    ServerGateProbe::Absent
}

#[cfg(not(target_arch = "wasm32"))]
pub fn recover_from_expired_server_session() {}

#[cfg(not(target_arch = "wasm32"))]
pub fn auth_state() -> WebAuthState {
    WebAuthState::Authenticated
}

#[cfg(target_arch = "wasm32")]
pub fn setup_credentials(username: &str, password: &str) -> Result<(), String> {
    validate_credentials(username, password)?;
    let credentials = StoredCredentials::new(username.trim().to_string(), password);
    browser::local_storage_set(AUTH_CONFIG_KEY, &credentials.encode())?;
    browser::session_storage_set(AUTH_SESSION_KEY, &credentials.session_token())?;
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn setup_credentials(_username: &str, _password: &str) -> Result<(), String> {
    Ok(())
}

#[cfg(target_arch = "wasm32")]
pub fn login(username: &str, password: &str) -> Result<(), String> {
    let credentials = load_credentials().ok_or_else(|| "尚未设置登录凭据。".to_string())?;
    if credentials.username != username.trim() || !credentials.verify(password) {
        return Err("用户名或密码错误。".to_string());
    }
    browser::session_storage_set(AUTH_SESSION_KEY, &credentials.session_token())?;
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn login(_username: &str, _password: &str) -> Result<(), String> {
    Ok(())
}

#[cfg(target_arch = "wasm32")]
pub fn configured_username() -> Option<String> {
    load_credentials().map(|credentials| credentials.username)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn configured_username() -> Option<String> {
    None
}

#[cfg(target_arch = "wasm32")]
fn validate_credentials(username: &str, password: &str) -> Result<(), String> {
    let username = username.trim();
    if username.is_empty() {
        return Err("用户名不能为空。".to_string());
    }
    if username.len() < 3 {
        return Err("用户名至少需要 3 个字符。".to_string());
    }
    if password.len() < 8 {
        return Err("密码至少需要 8 个字符。".to_string());
    }
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn generate_salt(username: &str) -> String {
    let seed = format!("{}:{}", username.trim(), browser::browser_now_millis());
    URL_SAFE_NO_PAD.encode(Sha256::digest(seed.as_bytes()))
}

#[cfg(target_arch = "wasm32")]
fn hash_password(username: &str, password: &str, salt: &str) -> String {
    let normalized = format!("{}\n{}\n{}", username.trim(), password, salt);
    URL_SAFE_NO_PAD.encode(Sha256::digest(normalized.as_bytes()))
}

#[cfg(target_arch = "wasm32")]
fn load_credentials() -> Option<StoredCredentials> {
    StoredCredentials::decode(&browser::local_storage_get(AUTH_CONFIG_KEY)?)
}

#[cfg(any(target_arch = "wasm32", test))]
fn is_local_protection_host(hostname: &str) -> bool {
    let hostname = hostname.trim().to_ascii_lowercase();
    matches!(hostname.as_str(), "localhost" | "127.0.0.1" | "::1" | "[::1]")
}

#[cfg(test)]
mod tests {
    use super::is_local_protection_host;

    #[test]
    fn local_web_auth_only_applies_to_loopback_hosts() {
        assert!(is_local_protection_host("localhost"));
        assert!(is_local_protection_host("LOCALHOST"));
        assert!(is_local_protection_host("127.0.0.1"));
        assert!(is_local_protection_host("::1"));
        assert!(is_local_protection_host("[::1]"));

        assert!(!is_local_protection_host("rss-reader.example.com"));
        assert!(!is_local_protection_host("192.168.1.10"));
        assert!(!is_local_protection_host("0.0.0.0"));
    }
}
