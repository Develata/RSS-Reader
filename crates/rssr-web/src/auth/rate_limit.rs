use std::{collections::HashMap, net::IpAddr};

use axum::http::HeaderMap;
use time::OffsetDateTime;
use tracing::debug;

use super::{
    AppState,
    config::{AuthConfig, LoginRateLimit},
    session::{DEVICE_COOKIE, device_nonce, extract_cookie},
};

/// 计数表的容量上限。新键只能来自不同的客户端地址或已知设备，正常部署远远到不了这里。
/// 表满后，没有单独计数的客户端地址共用一份溢出计数（[`OVERFLOW_KEY`]）：内存不随攻击者的
/// 来源数增长，这些来源也不会因此免于限速。已知设备照常单独计数：设备标记由服务端签发，
/// 攻击者无法批量制造（审计 F1）。
pub(crate) const MAX_TRACKED_LOGIN_KEYS: usize = 4096;
const OVERFLOW_KEY: &str = "overflow";
const DEVICE_KEY_PREFIX: &str = "device|";

#[derive(Clone, Debug)]
pub(crate) struct LoginThrottleState {
    pub(crate) failures: u32,
    pub(crate) window_started_at: OffsetDateTime,
    pub(crate) blocked_until: Option<OffsetDateTime>,
}

/// 登录失败计数的键（审计 F1）。不看用户名：任何用户名的失败都记在同一个来源上，否则只有
/// 真实用户名会触发限速，限速本身就成了判断用户名是否存在的信号。
///
/// - 带有效已知设备标记的请求按设备计数：只有它自己的失败才会锁住它。
/// - 其余请求按客户端地址计数：默认取 TCP 对端地址；只有显式信任代理头时才取转发地址。
pub(crate) fn rate_limit_key(
    config: &AuthConfig,
    headers: &HeaderMap,
    peer: Option<IpAddr>,
) -> String {
    if let Some(nonce) =
        extract_cookie(headers, DEVICE_COOKIE).and_then(|value| device_nonce(config, value))
    {
        return format!("{DEVICE_KEY_PREFIX}{nonce}");
    }
    let client = if config.trust_proxy_headers { forwarded_ip(headers) } else { None };
    let client = client.map(str::to_string).or_else(|| peer.map(|ip| ip.to_string()));
    format!("client|{}", client.as_deref().unwrap_or("unknown"))
}

pub(crate) fn is_device_key(key: &str) -> bool {
    key.starts_with(DEVICE_KEY_PREFIX)
}

/// 实际记账的条目：已有条目或还有空位时就是 `key` 自己；表满时新的客户端地址落到溢出计数。
fn bucket<'a>(throttle: &HashMap<String, LoginThrottleState>, key: &'a str) -> &'a str {
    if is_device_key(key) || throttle.contains_key(key) || throttle.len() < MAX_TRACKED_LOGIN_KEYS {
        key
    } else {
        OVERFLOW_KEY
    }
}

pub(crate) async fn login_attempt_is_blocked(state: &AppState, key: &str) -> bool {
    let now = OffsetDateTime::now_utc();
    let mut throttle = state.login_throttle.lock().await;
    cleanup_rate_limit_entries(&mut throttle, now, &state.config.login_rate_limit);
    throttle
        .get(bucket(&throttle, key))
        .and_then(|entry| entry.blocked_until)
        .is_some_and(|blocked_until| blocked_until > now)
}

pub(crate) async fn record_login_failure(state: &AppState, key: &str) -> bool {
    let now = OffsetDateTime::now_utc();
    let mut throttle = state.login_throttle.lock().await;
    cleanup_rate_limit_entries(&mut throttle, now, &state.config.login_rate_limit);
    let bucket = bucket(&throttle, key);
    if bucket == OVERFLOW_KEY {
        debug!("登录失败计数表已满，新的客户端地址共用溢出计数");
    }
    let entry = throttle.entry(bucket.to_string()).or_insert(LoginThrottleState {
        failures: 0,
        window_started_at: now,
        blocked_until: None,
    });

    if now - entry.window_started_at > state.config.login_rate_limit.window {
        entry.failures = 0;
        entry.window_started_at = now;
        entry.blocked_until = None;
    }

    entry.failures += 1;
    if entry.failures >= state.config.login_rate_limit.max_failures {
        entry.blocked_until = Some(now + state.config.login_rate_limit.block_for);
        true
    } else {
        false
    }
}

pub(crate) async fn clear_login_failures(state: &AppState, key: &str) {
    let mut throttle = state.login_throttle.lock().await;
    throttle.remove(key);
}

pub(crate) fn cleanup_rate_limit_entries(
    throttle: &mut HashMap<String, LoginThrottleState>,
    now: OffsetDateTime,
    config: &LoginRateLimit,
) {
    throttle.retain(|_, entry| {
        if let Some(blocked_until) = entry.blocked_until
            && blocked_until > now
        {
            return true;
        }
        now - entry.window_started_at <= config.window
    });
}

/// 可信反向代理会把它看到的对端地址**追加**到 `X-Forwarded-For` 末尾，更靠左的项是客户端
/// 自己填的，可以随意伪造（每次换一个就能绕过限速）。因此取最后一个头的最右一项：只有一层
/// 可信代理时，它就是真实客户端地址。没有该头时退回 `X-Real-IP`。
fn forwarded_ip(headers: &HeaderMap) -> Option<&str> {
    headers
        .get_all("x-forwarded-for")
        .iter()
        .next_back()
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.rsplit(',').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|value| value.to_str().ok())
                .map(str::trim)
                .filter(|value| !value.is_empty())
        })
}
