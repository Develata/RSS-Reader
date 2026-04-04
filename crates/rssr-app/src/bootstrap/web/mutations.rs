use anyhow::Context;
use rssr_domain::{UserSettings, normalize_feed_url};
use url::Url;

use super::{
    AppServices,
    config::validate_settings,
    state::{PersistedFeed, save_state_snapshot},
    web_now_utc,
};

pub(super) fn set_read(services: &AppServices, entry_id: i64, is_read: bool) -> anyhow::Result<()> {
    let snapshot = {
        let mut state = services.state.lock().expect("lock state");
        let now = web_now_utc();
        let entry =
            state.entries.iter_mut().find(|entry| entry.id == entry_id).context("文章不存在")?;
        entry.is_read = is_read;
        entry.read_at = is_read.then_some(now);
        entry.updated_at = now;
        state.clone()
    };
    save_state_snapshot(snapshot)
}

pub(super) fn set_starred(
    services: &AppServices,
    entry_id: i64,
    is_starred: bool,
) -> anyhow::Result<()> {
    let snapshot = {
        let mut state = services.state.lock().expect("lock state");
        let now = web_now_utc();
        let entry =
            state.entries.iter_mut().find(|entry| entry.id == entry_id).context("文章不存在")?;
        entry.is_starred = is_starred;
        entry.starred_at = is_starred.then_some(now);
        entry.updated_at = now;
        state.clone()
    };
    save_state_snapshot(snapshot)
}

pub(super) fn save_settings(services: &AppServices, settings: &UserSettings) -> anyhow::Result<()> {
    validate_settings(settings)?;
    let snapshot = {
        let mut state = services.state.lock().expect("lock state");
        state.settings = settings.clone();
        state.clone()
    };
    save_state_snapshot(snapshot)
}

pub(super) fn remember_last_opened_feed_id(
    services: &AppServices,
    feed_id: i64,
) -> anyhow::Result<()> {
    let snapshot = {
        let mut state = services.state.lock().expect("lock state");
        state.last_opened_feed_id = Some(feed_id);
        state.clone()
    };
    save_state_snapshot(snapshot)
}

pub(super) fn add_subscription(services: &AppServices, url: &Url) -> anyhow::Result<i64> {
    let normalized = normalize_feed_url(url);
    let feed_id = {
        let mut state = services.state.lock().expect("lock state");
        let now = web_now_utc();
        if let Some(feed) = state.feeds.iter_mut().find(|feed| feed.url == normalized.as_str()) {
            feed.is_deleted = false;
            feed.updated_at = now;
        } else {
            state.next_feed_id += 1;
            let feed_id = state.next_feed_id;
            state.feeds.push(PersistedFeed {
                id: feed_id,
                url: normalized.to_string(),
                title: None,
                site_url: None,
                description: None,
                icon_url: None,
                folder: None,
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
        let feed_id = state
            .feeds
            .iter()
            .find(|feed| feed.url == normalized.as_str())
            .expect("feed exists")
            .id;
        let snapshot = state.clone();
        drop(state);
        save_state_snapshot(snapshot)?;
        feed_id
    };
    Ok(feed_id)
}

pub(super) fn remove_feed(services: &AppServices, feed_id: i64) -> anyhow::Result<()> {
    let snapshot = {
        let mut state = services.state.lock().expect("lock state");
        let feed = state
            .feeds
            .iter_mut()
            .find(|feed| feed.id == feed_id && !feed.is_deleted)
            .context("订阅不存在")?;
        feed.is_deleted = true;
        feed.updated_at = web_now_utc();
        state.entries.retain(|entry| entry.feed_id != feed_id);
        if state.last_opened_feed_id == Some(feed_id) {
            state.last_opened_feed_id = None;
        }
        state.clone()
    };
    save_state_snapshot(snapshot)
}
