use std::sync::{Arc, Mutex};

use anyhow::Result;
use rssr_application::AppStatePort;
use rssr_domain::{AppStateRepository, AppStateSnapshot};

use crate::application_adapters::browser::state::{BrowserState, save_app_state_slice};

use super::shared::map_persistence_error;

#[derive(Clone)]
pub struct BrowserAppStateAdapter {
    state: Arc<Mutex<BrowserState>>,
}

impl BrowserAppStateAdapter {
    pub fn new(state: Arc<Mutex<BrowserState>>) -> Self {
        Self { state }
    }

    pub fn load_snapshot(&self) -> Result<AppStateSnapshot> {
        Ok(self.state.lock().expect("lock state").app_state.clone())
    }

    pub fn save_snapshot(&self, app_state: &AppStateSnapshot) -> Result<()> {
        let persisted = {
            let mut state = self.state.lock().expect("lock state");
            state.app_state = app_state.clone();
            state.app_state.clone()
        };

        save_app_state_slice(&persisted)
    }

    fn clear_last_opened_feed_if_matches_impl(&self, feed_id: i64) -> Result<()> {
        let persisted = {
            let mut state = self.state.lock().expect("lock state");
            if state.app_state.last_opened_feed_id != Some(feed_id) {
                return Ok(());
            }
            state.app_state.last_opened_feed_id = None;
            state.app_state.clone()
        };

        save_app_state_slice(&persisted)
    }
}

#[async_trait::async_trait]
impl AppStateRepository for BrowserAppStateAdapter {
    async fn load(&self) -> rssr_domain::Result<AppStateSnapshot> {
        self.load_snapshot().map_err(map_persistence_error)
    }

    async fn save(&self, state: &AppStateSnapshot) -> rssr_domain::Result<()> {
        self.save_snapshot(state).map_err(map_persistence_error)
    }
}

#[async_trait::async_trait]
impl AppStatePort for BrowserAppStateAdapter {
    async fn clear_last_opened_feed_if_matches(&self, feed_id: i64) -> Result<()> {
        self.clear_last_opened_feed_if_matches_impl(feed_id)
    }
}
