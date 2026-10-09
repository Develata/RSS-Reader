use anyhow::{Context, Result};
use axum::http::{HeaderMap, header};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, Mac};
use rand_core::RngCore;
use sha2::Sha256;
use time::OffsetDateTime;

use super::config::AuthConfig;

pub(crate) const SESSION_COOKIE: &str = "rssr_web_session";
pub(crate) const GATE_COOKIE: &str = "rssr_web_gate";
/// 登录成功过的浏览器所持有的“已知设备”标记，只发给 `/login`。
pub(crate) const DEVICE_COOKIE: &str = "rssr_web_device";
/// 浏览器允许的 Cookie 最长寿命约为 400 天（RFC 6265bis §5.5）。
const DEVICE_COOKIE_MAX_AGE_SECONDS: i64 = 400 * 24 * 60 * 60;

type HmacSha256 = Hmac<Sha256>;

/// 签发已知设备标记：随机 nonce 加上对 `rssr-device-v1:用户名:nonce` 的 HMAC。
///
/// 登录限速据此把这台浏览器的失败计数与其他客户端分开（OWASP “device cookies”）：远端
/// 攻击者即使与所有者共用同一个客户端地址（例如反向代理后面），也锁不住已知设备（审计 F1）。
pub(crate) fn build_device_token(config: &AuthConfig) -> Result<String> {
    let mut nonce = [0_u8; 16];
    rand_core::OsRng.fill_bytes(&mut nonce);
    let nonce = URL_SAFE_NO_PAD.encode(nonce);
    let signature = device_mac(config, &nonce)?.finalize().into_bytes();
    Ok(format!("{nonce}.{}", URL_SAFE_NO_PAD.encode(signature)))
}

/// 标记签名有效时返回其中的 nonce。
pub(crate) fn device_nonce<'a>(config: &AuthConfig, cookie_value: &'a str) -> Option<&'a str> {
    let (nonce, signature) = cookie_value.split_once('.')?;
    let signature = URL_SAFE_NO_PAD.decode(signature).ok()?;
    device_mac(config, nonce).ok()?.verify_slice(&signature).ok()?;
    Some(nonce)
}

fn device_mac(config: &AuthConfig, nonce: &str) -> Result<HmacSha256> {
    let mut mac =
        HmacSha256::new_from_slice(&config.session_secret).context("初始化设备标记签名器失败")?;
    mac.update(format!("rssr-device-v1:{}:{nonce}", config.username).as_bytes());
    Ok(mac)
}

pub(crate) fn device_cookie_header(token: &str, config: &AuthConfig) -> String {
    let secure = if config.secure_cookie { "; Secure" } else { "" };
    format!(
        "{DEVICE_COOKIE}={token}; Path=/login; HttpOnly; SameSite=Strict; Max-Age={DEVICE_COOKIE_MAX_AGE_SECONDS}{secure}"
    )
}

pub(crate) fn build_session_token(config: &AuthConfig, expires_at: i64) -> Result<String> {
    let payload = format!("{}:{expires_at}", config.username);
    let mut mac =
        HmacSha256::new_from_slice(&config.session_secret).context("初始化会话签名器失败")?;
    mac.update(payload.as_bytes());
    let signature = mac.finalize().into_bytes();
    Ok(format!(
        "{}.{}",
        URL_SAFE_NO_PAD.encode(payload.as_bytes()),
        URL_SAFE_NO_PAD.encode(signature)
    ))
}

pub(crate) fn session_is_valid(config: &AuthConfig, cookie_value: &str) -> bool {
    let Some((payload_part, signature_part)) = cookie_value.split_once('.') else {
        return false;
    };
    let Ok(payload_bytes) = URL_SAFE_NO_PAD.decode(payload_part) else {
        return false;
    };
    let Ok(signature_bytes) = URL_SAFE_NO_PAD.decode(signature_part) else {
        return false;
    };
    let Ok(payload) = String::from_utf8(payload_bytes) else {
        return false;
    };
    let Some((username, expires_raw)) = payload.rsplit_once(':') else {
        return false;
    };
    if username != config.username {
        return false;
    }
    let Ok(expires_at) = expires_raw.parse::<i64>() else {
        return false;
    };
    if OffsetDateTime::now_utc().unix_timestamp() > expires_at {
        return false;
    }

    let Ok(mut mac) = HmacSha256::new_from_slice(&config.session_secret) else {
        return false;
    };
    mac.update(payload.as_bytes());
    mac.verify_slice(&signature_bytes).is_ok()
}

pub(crate) fn session_cookie_header(token: &str, config: &AuthConfig) -> String {
    let max_age = config.session_ttl.whole_seconds();
    let secure = if config.secure_cookie { "; Secure" } else { "" };
    format!("{SESSION_COOKIE}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={max_age}{secure}")
}

pub(crate) fn logout_cookie_header(config: &AuthConfig) -> String {
    let secure = if config.secure_cookie { "; Secure" } else { "" };
    format!("{SESSION_COOKIE}=deleted; Path=/; HttpOnly; SameSite=Lax; Max-Age=0{secure}")
}

pub(crate) fn gate_cookie_header(config: &AuthConfig) -> String {
    let max_age = config.session_ttl.whole_seconds();
    let secure = if config.secure_cookie { "; Secure" } else { "" };
    format!("{GATE_COOKIE}=1; Path=/; SameSite=Lax; Max-Age={max_age}{secure}")
}

pub(crate) fn logout_gate_cookie_header(config: &AuthConfig) -> String {
    let secure = if config.secure_cookie { "; Secure" } else { "" };
    format!("{GATE_COOKIE}=deleted; Path=/; SameSite=Lax; Max-Age=0{secure}")
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
