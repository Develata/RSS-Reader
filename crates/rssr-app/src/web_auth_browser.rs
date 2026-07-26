#[cfg(target_arch = "wasm32")]
use js_sys::wasm_bindgen::JsCast;

#[cfg(target_arch = "wasm32")]
const SERVER_GATE_COOKIE: &str = "rssr_web_gate";

#[cfg(target_arch = "wasm32")]
pub(super) fn browser_origin() -> Option<String> {
    let window = web_sys::window()?;
    window.location().origin().ok()
}

#[cfg(target_arch = "wasm32")]
pub(super) fn browser_now_millis() -> f64 {
    js_sys::Date::now()
}

#[cfg(target_arch = "wasm32")]
pub(super) fn local_storage_get(key: &str) -> Option<String> {
    let window = web_sys::window()?;
    let storage = window.local_storage().ok()??;
    storage.get_item(key).ok()?
}

#[cfg(target_arch = "wasm32")]
pub(super) fn local_storage_set(key: &str, value: &str) -> Result<(), String> {
    let window = web_sys::window().ok_or_else(|| "浏览器窗口不可用。".to_string())?;
    let storage = window
        .local_storage()
        .map_err(|err| format!("读取本地存储失败：{err:?}"))?
        .ok_or_else(|| "浏览器不支持 localStorage。".to_string())?;
    storage.set_item(key, value).map_err(|err| format!("写入本地存储失败：{err:?}"))
}

#[cfg(target_arch = "wasm32")]
pub(super) fn session_storage_get(key: &str) -> Option<String> {
    let window = web_sys::window()?;
    let storage = window.session_storage().ok()??;
    storage.get_item(key).ok()?
}

#[cfg(target_arch = "wasm32")]
pub(super) fn session_storage_set(key: &str, value: &str) -> Result<(), String> {
    let window = web_sys::window().ok_or_else(|| "浏览器窗口不可用。".to_string())?;
    let storage = window
        .session_storage()
        .map_err(|err| format!("读取会话存储失败：{err:?}"))?
        .ok_or_else(|| "浏览器不支持 sessionStorage。".to_string())?;
    storage.set_item(key, value).map_err(|err| format!("写入会话存储失败：{err:?}"))
}

#[cfg(target_arch = "wasm32")]
pub(super) fn server_gate_present() -> bool {
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

#[cfg(target_arch = "wasm32")]
pub(super) fn local_web_auth_enabled() -> bool {
    let Some(window) = web_sys::window() else {
        return false;
    };
    let Ok(hostname) = window.location().hostname() else {
        return false;
    };
    super::is_local_protection_host(&hostname)
}

/// 整页跳到服务端登录页，并带上当前路由。
///
/// 服务端会话过期时必须走这条路：本地浏览器门禁与服务端登录是两套互不相关的凭据，
/// 把用户引去创建本地凭据既解决不了问题，还会留下一组永远用不上的 localStorage 凭据。
///
/// 带 `next` 是为了与服务端自己的 302 行为一致（`rssr-web` 的 `require_auth` 会保留目标路径）；
/// 不带的话同一个「会话过期」事件，走服务端重定向能回到原页面、走这里却被扔回首页。
/// 服务端的 `sanitize_next` 会拒掉 `//` 与外域，所以这里不引入新的跳转风险。
#[cfg(target_arch = "wasm32")]
pub(super) fn redirect_to_server_login() {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(origin) = browser_origin() else {
        return;
    };

    let location = window.location();
    let next = match (location.pathname(), location.search()) {
        (Ok(path), Ok(query)) => format!("{path}{query}"),
        (Ok(path), Err(_)) => path,
        _ => String::new(),
    };

    let target = if next.is_empty() || next == "/" {
        format!("{origin}/login")
    } else {
        format!("{origin}/login?next={}", urlencoding::encode(&next))
    };
    let _ = location.set_href(&target);
}
