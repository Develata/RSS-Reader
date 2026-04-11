use std::{
    sync::{Arc, atomic::Ordering},
    time::Duration,
};

use rssr_infra::application_adapters::browser::now_utc;
use wasm_bindgen_futures::spawn_local;

use super::AppServices;

pub(super) fn ensure_auto_refresh_started(services: &Arc<AppServices>) {
    if services.auto_refresh_started.swap(true, Ordering::SeqCst) {
        return;
    }

    let services = Arc::clone(services);
    let refresh = services.host_capabilities().refresh;
    spawn_local(async move {
        let mut last_refresh_started_at = None;

        loop {
            let settings = match services.use_cases().settings_service.load().await {
                Ok(settings) => settings,
                Err(error) => {
                    tracing::warn!(error = %error, "读取自动刷新设置失败，稍后重试");
                    gloo_timers::future::sleep(Duration::from_secs(30)).await;
                    continue;
                }
            };

            let now = now_utc();
            if super::super::should_trigger_auto_refresh(
                last_refresh_started_at,
                settings.refresh_interval_minutes,
                now,
            ) {
                tracing::info!(
                    refresh_interval_minutes = settings.refresh_interval_minutes,
                    "触发后台自动刷新全部订阅"
                );
                if let Err(error) = refresh.refresh_all().await {
                    tracing::warn!(error = %error, "后台自动刷新失败");
                }
                last_refresh_started_at = Some(now);
            }

            let wait_for = super::super::auto_refresh_wait_duration(
                last_refresh_started_at,
                settings.refresh_interval_minutes,
                now_utc(),
            );
            gloo_timers::future::sleep(wait_for).await;
        }
    });
}
