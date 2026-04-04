use std::collections::HashSet;

use anyhow::Context;
use reqwest::StatusCode;
use rssr_domain::{ConfigFeed, ConfigPackage, normalize_feed_url};
use url::Url;

use super::{
    AppServices,
    config::{decode_opml, encode_opml, import_field, remote_url, validate_config_package},
    state::{PersistedFeed, save_state_snapshot},
    web_now_utc,
};

pub(super) fn export_config_json(services: &AppServices) -> anyhow::Result<String> {
    Ok(serde_json::to_string_pretty(&export_config(services)?)?)
}

pub(super) fn import_config_json(services: &AppServices, raw: &str) -> anyhow::Result<()> {
    let package: ConfigPackage = serde_json::from_str(raw)?;
    validate_config_package(&package)?;

    let snapshot = {
        let mut state = services.state.lock().expect("lock state");
        let current_urls = state
            .feeds
            .iter()
            .filter(|feed| !feed.is_deleted)
            .map(|feed| (feed.id, feed.url.clone()))
            .collect::<Vec<_>>();
        let mut imported_urls = HashSet::new();

        for feed in package.feeds {
            let url = normalize_feed_url(
                &Url::parse(&feed.url).with_context(|| format!("无效的订阅 URL：{}", feed.url))?,
            );
            imported_urls.insert(url.to_string());
            upsert_imported_feed(&mut state, &url, feed.title, feed.folder);
        }

        let removed_feed_ids = current_urls
            .into_iter()
            .filter_map(|(id, url)| match Url::parse(&url) {
                Ok(parsed) => {
                    let normalized = normalize_feed_url(&parsed);
                    (!imported_urls.contains(normalized.as_str())).then_some(id)
                }
                Err(error) => {
                    tracing::warn!(
                        feed_id = id,
                        invalid_url = %url,
                        error = %error,
                        "导入配置时发现损坏的已持久化订阅 URL，已将其标记为移除"
                    );
                    Some(id)
                }
            })
            .collect::<Vec<_>>();
        for feed_id in &removed_feed_ids {
            if let Some(feed) = state.feeds.iter_mut().find(|feed| feed.id == *feed_id) {
                feed.is_deleted = true;
            }
        }
        state.entries.retain(|entry| !removed_feed_ids.contains(&entry.feed_id));
        state.settings = package.settings;
        state.clone()
    };
    save_state_snapshot(snapshot)
}

pub(super) fn export_opml(services: &AppServices) -> anyhow::Result<String> {
    encode_opml(&export_config(services)?.feeds)
}

pub(super) fn import_opml(services: &AppServices, raw: &str) -> anyhow::Result<()> {
    let feeds = decode_opml(raw)?;
    let snapshot = {
        let mut state = services.state.lock().expect("lock state");
        for feed in feeds {
            let url = normalize_feed_url(
                &Url::parse(&feed.url).with_context(|| format!("无效的订阅 URL：{}", feed.url))?,
            );
            upsert_imported_feed(&mut state, &url, feed.title, feed.folder);
        }
        state.clone()
    };
    save_state_snapshot(snapshot)
}

pub(super) async fn push_remote_config(
    services: &AppServices,
    endpoint: &str,
    remote_path: &str,
) -> anyhow::Result<()> {
    let url = remote_url(endpoint, remote_path)?;
    services
        .client
        .put(url)
        .header("content-type", "application/json")
        .body(export_config_json(services)?)
        .send()
        .await
        .context("上传配置到 WebDAV 失败")?
        .error_for_status()
        .context("WebDAV 上传失败")?;
    Ok(())
}

pub(super) async fn pull_remote_config(
    services: &AppServices,
    endpoint: &str,
    remote_path: &str,
) -> anyhow::Result<bool> {
    let response = services
        .client
        .get(remote_url(endpoint, remote_path)?)
        .send()
        .await
        .context("从 WebDAV 下载配置失败")?;
    if response.status() == StatusCode::NOT_FOUND {
        return Ok(false);
    }
    let raw = response.error_for_status().context("WebDAV 下载失败")?.text().await?;
    import_config_json(services, &raw)?;
    Ok(true)
}

fn export_config(services: &AppServices) -> anyhow::Result<ConfigPackage> {
    let state = services.state.lock().expect("lock state");
    Ok(ConfigPackage {
        version: 1,
        exported_at: web_now_utc(),
        feeds: state
            .feeds
            .iter()
            .filter(|feed| !feed.is_deleted)
            .map(|feed| ConfigFeed {
                url: feed.url.clone(),
                title: feed.title.clone(),
                folder: feed.folder.clone(),
            })
            .collect(),
        settings: state.settings.clone(),
    })
}

fn upsert_imported_feed(
    state: &mut super::state::PersistedState,
    url: &Url,
    title: Option<String>,
    folder: Option<String>,
) {
    let now = web_now_utc();
    if let Some(existing) = state.feeds.iter_mut().find(|current| current.url == url.as_str()) {
        existing.title = import_field(title, true);
        existing.folder = import_field(folder, true);
        existing.is_deleted = false;
        existing.updated_at = now;
    } else {
        state.next_feed_id += 1;
        let feed_id = state.next_feed_id;
        state.feeds.push(PersistedFeed {
            id: feed_id,
            url: url.to_string(),
            title,
            site_url: None,
            description: None,
            icon_url: None,
            folder,
            etag: None,
            last_modified: None,
            last_fetched_at: None,
            last_success_at: None,
            fetch_error: None,
            is_deleted: false,
            created_at: now,
            updated_at: now,
        });
    }
}
