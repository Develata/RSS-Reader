use std::collections::HashMap;

use axum::http::HeaderMap;
use time::OffsetDateTime;

use super::{
    AppState,
    config::{AuthConfig, LoginRateLimit},
};

#[derive(Clone, Debug)]
pub(crate) struct LoginThrottleState {
    pub(crate) failures: u32,
    pub(crate) window_started_at: OffsetDateTime,
    pub(crate) blocked_until: Option<OffsetDateTime>,
}

pub(crate) fn rate_limit_key(config: &AuthConfig, headers: &HeaderMap, username: &str) -> String {
    let client = forwarded_ip(config, headers).unwrap_or("direct");
    format!("{}|{}", client, username.trim().to_ascii_lowercase())
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

fn forwarded_ip<'a>(config: &AuthConfig, headers: &'a HeaderMap) -> Option<&'a str> {
    if !config.trust_proxy_headers {
        return None;
    }

    headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
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
