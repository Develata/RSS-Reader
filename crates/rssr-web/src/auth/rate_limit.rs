use std::{collections::HashMap, net::IpAddr};

use axum::http::HeaderMap;
use time::OffsetDateTime;
use tracing::warn;

use super::{
    AppState,
    config::{AuthConfig, LoginRateLimit},
    session::{DEVICE_COOKIE, device_nonce, extract_cookie},
};

/// 计数表的容量上限。新键只能来自不同的客户端地址或已知设备，正常部署远远到不了这里；
/// 到达上限后不再记录新键，避免内存随攻击者的来源数无界增长。
const MAX_TRACKED_LOGIN_KEYS: usize = 4096;

#[derive(Clone, Debug)]
pub(crate) struct LoginThrottleState {
    pub(crate) failures: u32,
    pub(crate) window_started_at: OffsetDateTime,
    pub(crate) blocked_until: Option<OffsetDateTime>,
}

/// 登录失败计数的键（审计 F1）。
///
/// - 用户名不是配置的那个时返回 `None`：这样的尝试不可能成功，不必为它分配计数键。
/// - 带有效已知设备标记的请求按设备计数：只有它自己的失败才会锁住它。
/// - 其余请求按客户端地址计数：默认取 TCP 对端地址；只有显式信任代理头时才取转发地址。
pub(crate) fn rate_limit_key(
    config: &AuthConfig,
    headers: &HeaderMap,
    peer: Option<IpAddr>,
    username: &str,
) -> Option<String> {
    if username.trim() != config.username {
        return None;
    }
    if let Some(nonce) =
        extract_cookie(headers, DEVICE_COOKIE).and_then(|value| device_nonce(config, value))
    {
        return Some(format!("device|{nonce}"));
    }
    let client = if config.trust_proxy_headers { forwarded_ip(headers) } else { None };
    let client = client.map(str::to_string).or_else(|| peer.map(|ip| ip.to_string()));
    Some(format!("client|{}", client.as_deref().unwrap_or("unknown")))
}

pub(crate) async fn login_attempt_is_blocked(state: &AppState, key: &str) -> bool {
    let now = OffsetDateTime::now_utc();
    let mut throttle = state.login_throttle.lock().await;
    cleanup_rate_limit_entries(&mut throttle, now, &state.config.login_rate_limit);
    throttle
        .get(key)
        .and_then(|entry| entry.blocked_until)
        .is_some_and(|blocked_until| blocked_until > now)
}

pub(crate) async fn record_login_failure(state: &AppState, key: &str) -> bool {
    let now = OffsetDateTime::now_utc();
    let mut throttle = state.login_throttle.lock().await;
    cleanup_rate_limit_entries(&mut throttle, now, &state.config.login_rate_limit);
    if !throttle.contains_key(key) && throttle.len() >= MAX_TRACKED_LOGIN_KEYS {
        warn!("登录失败计数表已满，暂不记录新的客户端");
        return false;
    }
    let entry = throttle.entry(key.to_string()).or_insert(LoginThrottleState {
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
