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

#[cfg(target_arch = "wasm32")]
pub async fn verify_server_gate() -> bool {
    if !browser::server_gate_present() {
        return false;
    }

    let Some(origin) = browser::browser_origin() else {
        return false;
    };
    let probe_url = format!("{origin}/session-probe");

    match reqwest::Client::new().get(probe_url).send().await {
        Ok(response) => response.status() == reqwest::StatusCode::NO_CONTENT,
        Err(_) => false,
    }
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
pub async fn verify_server_gate() -> bool {
    false
}

#[cfg(not(target_arch = "wasm32"))]
pub fn local_auth_state() -> WebAuthState {
    WebAuthState::Authenticated
}

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
