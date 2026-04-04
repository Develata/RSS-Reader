use std::{
    sync::{Arc, atomic::Ordering},
    time::Duration,
};

use anyhow::Context;
use reqwest::{StatusCode, header};
use wasm_bindgen_futures::spawn_local;

use super::{
    AppServices,
    feed::{parse_feed, web_fetch_feed_response},
    state::{save_state_snapshot, upsert_entries},
    web_now_utc,
};

pub(super) fn ensure_auto_refresh_started(services: &Arc<AppServices>) {
    if services.auto_refresh_started.swap(true, Ordering::SeqCst) {
        return;
    }

    let services = Arc::clone(services);
    spawn_local(async move {
        let mut last_refresh_started_at = None;

        loop {
            let settings = match services.load_settings().await {
                Ok(settings) => settings,
                Err(error) => {
                    tracing::warn!(error = %error, "读取自动刷新设置失败，稍后重试");
                    gloo_timers::future::sleep(Duration::from_secs(30)).await;
                    continue;
                }
            };

            let now = web_now_utc();
            if super::super::should_trigger_auto_refresh(
                last_refresh_started_at,
                settings.refresh_interval_minutes,
                now,
            ) {
                tracing::info!(
                    refresh_interval_minutes = settings.refresh_interval_minutes,
                    "触发后台自动刷新全部订阅"
                );
                if let Err(error) = refresh_all(&services).await {
                    tracing::warn!(error = %error, "后台自动刷新失败");
                }
                last_refresh_started_at = Some(now);
            }

            let wait_for = super::super::auto_refresh_wait_duration(
                last_refresh_started_at,
                settings.refresh_interval_minutes,
                web_now_utc(),
            );
            gloo_timers::future::sleep(wait_for).await;
        }
    });
}

pub(super) async fn refresh_all(services: &AppServices) -> anyhow::Result<()> {
    let feed_ids = {
        let state = services.state.lock().expect("lock state");
        state.feeds.iter().filter(|feed| !feed.is_deleted).map(|feed| feed.id).collect::<Vec<_>>()
    };
    let mut errors = Vec::new();
    for feed_id in feed_ids {
        if let Err(error) = refresh_feed(services, feed_id).await {
            errors.push(error.to_string());
        }
    }
    if !errors.is_empty() {
        anyhow::bail!("部分订阅刷新失败: {}", errors.join(" | "));
    }
    Ok(())
}

pub(super) async fn refresh_feed(services: &AppServices, feed_id: i64) -> anyhow::Result<()> {
    let url = {
        let state = services.state.lock().expect("lock state");
        let feed = state
            .feeds
            .iter()
            .find(|feed| feed.id == feed_id && !feed.is_deleted)
            .context("订阅不存在")?;
        feed.url.clone()
    };

    let response = match web_fetch_feed_response(&services.client, &url).await {
        Ok(response) => response,
        Err(error) => {
            let snapshot = {
                let mut state = services.state.lock().expect("lock state");
                let now = web_now_utc();
                if let Some(feed) = state.feeds.iter_mut().find(|feed| feed.id == feed_id) {
                    feed.last_fetched_at = Some(now);
                    feed.fetch_error = Some(format!("抓取订阅失败: {error}"));
                    feed.updated_at = now;
                }
                state.clone()
            };
            let _ = save_state_snapshot(snapshot);
            return Err(error);
        }
    };
    let metadata = (
        response
            .headers()
            .get(header::ETAG)
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned),
        response
            .headers()
            .get(header::LAST_MODIFIED)
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned),
    );

    if response.status() == StatusCode::NOT_MODIFIED {
        let snapshot = {
            let mut state = services.state.lock().expect("lock state");
            let now = web_now_utc();
            let feed =
                state.feeds.iter_mut().find(|feed| feed.id == feed_id).context("订阅不存在")?;
            feed.etag = metadata.0;
            feed.last_modified = metadata.1;
            feed.last_fetched_at = Some(now);
            feed.last_success_at = Some(now);
            feed.fetch_error = None;
            feed.updated_at = now;
            state.clone()
        };
        return save_state_snapshot(snapshot);
    }

    let body = match response.error_for_status() {
        Ok(response) => match response.text().await {
            Ok(body) => body,
            Err(error) => {
                let snapshot = {
                    let mut state = services.state.lock().expect("lock state");
                    let now = web_now_utc();
                    if let Some(feed) = state.feeds.iter_mut().find(|feed| feed.id == feed_id) {
                        feed.last_fetched_at = Some(now);
                        feed.fetch_error = Some(format!("读取 feed 响应正文失败: {error}"));
                        feed.updated_at = now;
                    }
                    state.clone()
                };
                let _ = save_state_snapshot(snapshot);
                return Err(error).context("读取 feed 响应正文失败");
            }
        },
        Err(error) => {
            let snapshot = {
                let mut state = services.state.lock().expect("lock state");
                let now = web_now_utc();
                if let Some(feed) = state.feeds.iter_mut().find(|feed| feed.id == feed_id) {
                    feed.last_fetched_at = Some(now);
                    feed.fetch_error = Some(format!("feed 抓取返回非成功状态: {error}"));
                    feed.updated_at = now;
                }
                state.clone()
            };
            let _ = save_state_snapshot(snapshot);
            return Err(error).context("feed 抓取返回非成功状态");
        }
    };
    let parsed = match parse_feed(&body) {
        Ok(parsed) => parsed,
        Err(error) => {
            let snapshot = {
                let mut state = services.state.lock().expect("lock state");
                let now = web_now_utc();
                if let Some(feed) = state.feeds.iter_mut().find(|feed| feed.id == feed_id) {
                    feed.last_fetched_at = Some(now);
                    feed.fetch_error = Some(format!("解析订阅失败: {error}"));
                    feed.updated_at = now;
                }
                state.clone()
            };
            let _ = save_state_snapshot(snapshot);
            return Err(error).context("解析订阅失败");
        }
    };

    let snapshot = {
        let mut state = services.state.lock().expect("lock state");
        let now = web_now_utc();
        let feed = state.feeds.iter_mut().find(|feed| feed.id == feed_id).context("订阅不存在")?;
        if parsed.title.is_some() {
            feed.title = parsed.title;
        }
        if parsed.site_url.is_some() {
            feed.site_url = parsed.site_url.map(|url| url.to_string());
        }
        if parsed.description.is_some() {
            feed.description = parsed.description;
        }
        feed.etag = metadata.0;
        feed.last_modified = metadata.1;
        feed.last_fetched_at = Some(now);
        feed.last_success_at = Some(now);
        feed.fetch_error = None;
        feed.updated_at = now;

        upsert_entries(&mut state, feed_id, parsed.entries)?;
        state.clone()
    };
    save_state_snapshot(snapshot)
}
