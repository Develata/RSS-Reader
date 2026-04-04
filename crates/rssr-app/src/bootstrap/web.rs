use std::sync::{Arc, Mutex, atomic::AtomicBool};

#[path = "web/config.rs"]
mod config;
#[path = "web/exchange.rs"]
mod exchange;
#[path = "web/feed.rs"]
mod feed;
#[path = "web/mutations.rs"]
mod mutations;
#[path = "web/query.rs"]
mod query;
#[path = "web/refresh.rs"]
mod refresh;
#[path = "web/state.rs"]
mod state;

use anyhow::Context;
use js_sys::Date;
pub use rssr_domain::EntryNavigation as ReaderNavigation;
use rssr_domain::{Entry, EntryQuery, EntrySummary, FeedSummary, UserSettings, normalize_feed_url};
use time::OffsetDateTime;
use tokio::sync::OnceCell;
use url::Url;

use self::{
    exchange::{
        export_config_json as export_exchange_json, export_opml as export_exchange_opml,
        import_config_json as import_exchange_json, import_opml as import_exchange_opml,
        pull_remote_config as pull_exchange_remote, push_remote_config as push_exchange_remote,
    },
    mutations::{
        add_subscription as add_subscription_state,
        remember_last_opened_feed_id as remember_feed_id, remove_feed as remove_feed_state,
        save_settings as save_settings_state, set_read as set_entry_read,
        set_starred as set_entry_starred,
    },
    query::{
        get_entry as query_get_entry, list_entries as query_list_entries,
        list_feeds as query_list_feeds, reader_navigation as query_reader_navigation,
    },
    refresh::{
        ensure_auto_refresh_started as start_auto_refresh, refresh_all as run_refresh_all,
        refresh_feed as run_refresh_feed,
    },
    state::{PersistedState, load_state},
};

static APP_SERVICES: OnceCell<Arc<AppServices>> = OnceCell::const_new();

pub struct AppServices {
    state: Mutex<PersistedState>,
    client: reqwest::Client,
    auto_refresh_started: AtomicBool,
}

impl AppServices {
    pub async fn shared() -> anyhow::Result<Arc<Self>> {
        APP_SERVICES
            .get_or_try_init(|| async {
                let loaded = load_state();
                if let Some(warning) = loaded.warning.as_deref() {
                    tracing::warn!(warning = warning, "Web 本地状态恢复时发现异常");
                }
                Ok(Arc::new(Self {
                    state: Mutex::new(loaded.state),
                    client: reqwest::Client::new(),
                    auto_refresh_started: AtomicBool::new(false),
                }))
            })
            .await
            .map(Arc::clone)
    }

    pub fn default_settings() -> UserSettings {
        UserSettings::default()
    }

    pub async fn list_feeds(&self) -> anyhow::Result<Vec<FeedSummary>> {
        let state = self.state.lock().expect("lock state");
        Ok(query_list_feeds(&state))
    }

    pub async fn list_entries(&self, query: &EntryQuery) -> anyhow::Result<Vec<EntrySummary>> {
        let state = self.state.lock().expect("lock state");
        Ok(query_list_entries(&state, query))
    }

    pub async fn get_entry(&self, entry_id: i64) -> anyhow::Result<Option<Entry>> {
        let state = self.state.lock().expect("lock state");
        query_get_entry(&state, entry_id)
    }

    pub async fn reader_navigation(
        &self,
        current_entry_id: i64,
    ) -> anyhow::Result<ReaderNavigation> {
        let state = self.state.lock().expect("lock state");
        Ok(query_reader_navigation(&state, current_entry_id))
    }

    pub async fn set_read(&self, entry_id: i64, is_read: bool) -> anyhow::Result<()> {
        set_entry_read(self, entry_id, is_read)
    }

    pub async fn set_starred(&self, entry_id: i64, is_starred: bool) -> anyhow::Result<()> {
        set_entry_starred(self, entry_id, is_starred)
    }

    pub async fn load_settings(&self) -> anyhow::Result<UserSettings> {
        Ok(self.state.lock().expect("lock state").settings.clone())
    }

    pub async fn save_settings(&self, settings: &UserSettings) -> anyhow::Result<()> {
        save_settings_state(self, settings)
    }

    pub async fn load_last_opened_feed_id(&self) -> anyhow::Result<Option<i64>> {
        Ok(self.state.lock().expect("lock state").last_opened_feed_id)
    }

    pub async fn remember_last_opened_feed_id(&self, feed_id: i64) -> anyhow::Result<()> {
        remember_feed_id(self, feed_id)
    }

    pub fn ensure_auto_refresh_started(self: &Arc<Self>) {
        start_auto_refresh(self);
    }

    pub async fn add_subscription(&self, raw_url: &str) -> anyhow::Result<()> {
        let url = normalize_feed_url(&Url::parse(raw_url).context("订阅 URL 不合法")?);
        let feed_id = add_subscription_state(self, &url)?;
        self.refresh_feed(feed_id).await.context("首次刷新订阅失败")
    }

    pub async fn remove_feed(&self, feed_id: i64) -> anyhow::Result<()> {
        remove_feed_state(self, feed_id)
    }

    pub async fn refresh_all(&self) -> anyhow::Result<()> {
        run_refresh_all(self).await
    }

    pub async fn refresh_feed(&self, feed_id: i64) -> anyhow::Result<()> {
        run_refresh_feed(self, feed_id).await
    }

    pub async fn export_config_json(&self) -> anyhow::Result<String> {
        export_exchange_json(self)
    }

    pub async fn import_config_json(&self, raw: &str) -> anyhow::Result<()> {
        import_exchange_json(self, raw)
    }

    pub async fn export_opml(&self) -> anyhow::Result<String> {
        export_exchange_opml(self)
    }

    pub async fn import_opml(&self, raw: &str) -> anyhow::Result<()> {
        import_exchange_opml(self, raw)
    }

    pub async fn push_remote_config(
        &self,
        endpoint: &str,
        remote_path: &str,
    ) -> anyhow::Result<()> {
        push_exchange_remote(self, endpoint, remote_path).await
    }

    pub async fn pull_remote_config(
        &self,
        endpoint: &str,
        remote_path: &str,
    ) -> anyhow::Result<bool> {
        pull_exchange_remote(self, endpoint, remote_path).await
    }
}

fn web_now_utc() -> OffsetDateTime {
    let millis = Date::now() as i128;
    OffsetDateTime::from_unix_timestamp_nanos(millis * 1_000_000)
        .expect("browser timestamp should fit in OffsetDateTime")
}

#[cfg(test)]
mod tests {
    use super::query::title_matches_search;

    #[test]
    fn web_title_search_is_case_insensitive() {
        assert!(title_matches_search("Roche Scales NVIDIA AI Factories", "sca"));
        assert!(title_matches_search("Roche Scales NVIDIA AI Factories", "SCA"));
        assert!(!title_matches_search("Roche Scales NVIDIA AI Factories", "xyz"));
    }
}
