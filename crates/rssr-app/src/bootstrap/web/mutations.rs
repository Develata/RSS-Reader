use anyhow::Context;
use rssr_domain::UserSettings;

use super::{AppServices, config::validate_settings, state::save_state_snapshot, web_now_utc};

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
