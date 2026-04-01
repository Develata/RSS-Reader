#[cfg(target_arch = "wasm32")]
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
#[cfg(target_arch = "wasm32")]
use js_sys::wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use sha2::{Digest, Sha256};

#[cfg(target_arch = "wasm32")]
const AUTH_CONFIG_KEY: &str = "rssr-web-auth-config-v1";
#[cfg(target_arch = "wasm32")]
const AUTH_SESSION_KEY: &str = "rssr-web-auth-session-v1";
#[cfg(target_arch = "wasm32")]
const SERVER_GATE_COOKIE: &str = "rssr_web_gate";

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
    if server_gate_present() {
        return WebAuthState::PendingServerProbe;
    }

    local_auth_state()
}

#[cfg(target_arch = "wasm32")]
pub async fn verify_server_gate() -> bool {
    if !server_gate_present() {
        return false;
    }

    let Some(window) = web_sys::window() else {
        return false;
    };
    let Ok(origin) = window.location().origin() else {
        return false;
    };
    let probe_url = format!("{origin}/session-probe");

    match reqwest::Client::new().get(probe_url).send().await {
        Ok(response) => response.status() == reqwest::StatusCode::NO_CONTENT,
        Err(_) => false,
    }
}

#[cfg(target_arch = "wasm32")]
pub fn has_server_gate_cookie() -> bool {
    server_gate_present()
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)]
pub fn has_server_gate_cookie() -> bool {
    false
}

#[cfg(target_arch = "wasm32")]
pub fn local_auth_state() -> WebAuthState {
    let Some(credentials) = load_credentials() else {
        return WebAuthState::NeedsSetup;
    };

    if session_storage_get(AUTH_SESSION_KEY).as_deref()
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
    local_storage_set(AUTH_CONFIG_KEY, &credentials.encode())?;
    session_storage_set(AUTH_SESSION_KEY, &credentials.session_token())?;
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
    session_storage_set(AUTH_SESSION_KEY, &credentials.session_token())?;
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
    #[cfg(target_arch = "wasm32")]
    {
        let seed = format!("{}:{}", username.trim(), js_sys::Date::now());
        return URL_SAFE_NO_PAD.encode(Sha256::digest(seed.as_bytes()));
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        URL_SAFE_NO_PAD.encode(Sha256::digest(username.trim().as_bytes()))
    }
}

#[cfg(target_arch = "wasm32")]
fn hash_password(username: &str, password: &str, salt: &str) -> String {
    let normalized = format!("{}\n{}\n{}", username.trim(), password, salt);
    URL_SAFE_NO_PAD.encode(Sha256::digest(normalized.as_bytes()))
}

#[cfg(target_arch = "wasm32")]
fn load_credentials() -> Option<StoredCredentials> {
    StoredCredentials::decode(&local_storage_get(AUTH_CONFIG_KEY)?)
}

#[cfg(target_arch = "wasm32")]
fn local_storage_get(key: &str) -> Option<String> {
    let window = web_sys::window()?;
    let storage = window.local_storage().ok()??;
    storage.get_item(key).ok()?
}

#[cfg(target_arch = "wasm32")]
fn local_storage_set(key: &str, value: &str) -> Result<(), String> {
    let window = web_sys::window().ok_or_else(|| "浏览器窗口不可用。".to_string())?;
    let storage = window
        .local_storage()
        .map_err(|err| format!("读取本地存储失败：{err:?}"))?
        .ok_or_else(|| "浏览器不支持 localStorage。".to_string())?;
    storage.set_item(key, value).map_err(|err| format!("写入本地存储失败：{err:?}"))
}

#[cfg(target_arch = "wasm32")]
fn session_storage_get(key: &str) -> Option<String> {
    let window = web_sys::window()?;
    let storage = window.session_storage().ok()??;
    storage.get_item(key).ok()?
}

#[cfg(target_arch = "wasm32")]
fn session_storage_set(key: &str, value: &str) -> Result<(), String> {
    let window = web_sys::window().ok_or_else(|| "浏览器窗口不可用。".to_string())?;
    let storage = window
        .session_storage()
        .map_err(|err| format!("读取会话存储失败：{err:?}"))?
        .ok_or_else(|| "浏览器不支持 sessionStorage。".to_string())?;
    storage.set_item(key, value).map_err(|err| format!("写入会话存储失败：{err:?}"))
}

#[cfg(target_arch = "wasm32")]
fn server_gate_present() -> bool {
    let Some(window) = web_sys::window() else {
        return false;
    };
    let Some(document) = window.document() else {
        return false;
    };
    let Ok(html_document) = document.dyn_into::<web_sys::HtmlDocument>() else {
        return false;
    };
    let Ok(cookie_string) = html_document.cookie() else {
        return false;
    };
    cookie_string
        .split(';')
        .map(str::trim)
        .filter_map(|entry| entry.split_once('='))
        .any(|(name, value)| name == SERVER_GATE_COOKIE && value == "1")
}
