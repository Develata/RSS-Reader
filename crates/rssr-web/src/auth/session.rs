//! Cookie：会话与已知设备标记。会话本身在认证库里（[`super::store`]）。

use axum::http::{HeaderMap, header};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, Mac};
use rand_core::RngCore;
use sha2::Sha256;

use super::store::ABSOLUTE;

pub(crate) const SESSION_COOKIE: &str = "rssr_web_session";
/// 登录成功过的浏览器所持有的“已知设备”标记，只发给 `/login`。
pub(crate) const DEVICE_COOKIE: &str = "rssr_web_device";
/// 浏览器允许的 Cookie 最长寿命约为 400 天（RFC 6265bis §5.5）。
const DEVICE_COOKIE_MAX_AGE_SECONDS: i64 = 400 * 24 * 60 * 60;

type HmacSha256 = Hmac<Sha256>;

/// 签发已知设备标记：随机 nonce 加上对 `rssr-device-v1:用户名:nonce` 的 HMAC，密钥存在认证库。
///
/// 登录限速据此把这台浏览器的失败计数与其他客户端分开（OWASP “device cookies”）：远端
/// 攻击者即使与所有者共用同一个客户端地址（例如反向代理后面），也锁不住已知设备（审计 F1）。
pub(crate) fn build_device_token(key: &[u8; 32], username: &str) -> String {
    let mut nonce = [0_u8; 16];
    rand_core::OsRng.fill_bytes(&mut nonce);
    let nonce = URL_SAFE_NO_PAD.encode(nonce);
    let signature = device_mac(key, username, &nonce).finalize().into_bytes();
    format!("{nonce}.{}", URL_SAFE_NO_PAD.encode(signature))
}

/// 标记签名有效时返回其中的 nonce。
pub(crate) fn device_nonce<'a>(
    key: &[u8; 32],
    username: &str,
    cookie_value: &'a str,
) -> Option<&'a str> {
    let (nonce, signature) = cookie_value.split_once('.')?;
    let signature = URL_SAFE_NO_PAD.decode(signature).ok()?;
    device_mac(key, username, nonce).verify_slice(&signature).ok()?;
    Some(nonce)
}

fn device_mac(key: &[u8; 32], username: &str, nonce: &str) -> HmacSha256 {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts keys of any length");
    mac.update(format!("rssr-device-v1:{username}:{nonce}").as_bytes());
    mac
}

fn secure_attribute(secure: bool) -> &'static str {
    if secure { "; Secure" } else { "" }
}

pub(crate) fn device_cookie_header(token: &str, secure: bool) -> String {
    format!(
        "{DEVICE_COOKIE}={token}; Path=/login; HttpOnly; SameSite=Strict; Max-Age={DEVICE_COOKIE_MAX_AGE_SECONDS}{}",
        secure_attribute(secure)
    )
}

/// 会话 cookie 的寿命与服务器的绝对上限相同：浏览器一直保留它（也可能提前清除），过期由
/// 服务器判定。
pub(crate) fn session_cookie_header(token: &str, secure: bool) -> String {
    format!(
        "{SESSION_COOKIE}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={ABSOLUTE}{}",
        secure_attribute(secure)
    )
}

pub(crate) fn logout_cookie_header(secure: bool) -> String {
    format!(
        "{SESSION_COOKIE}=deleted; Path=/; HttpOnly; SameSite=Lax; Max-Age=0{}",
        secure_attribute(secure)
    )
}

pub(crate) fn extract_cookie<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .filter_map(|part| {
            let trimmed = part.trim();
            let (cookie_name, cookie_value) = trimmed.split_once('=')?;
            (cookie_name == name).then_some(cookie_value)
        })
        .next()
}
