use std::sync::Arc;

use rssr_domain::{AppStateRepository, AppStateSnapshot, EntriesWorkspaceState};

#[derive(Clone)]
pub struct AppStateService {
    repository: Arc<dyn AppStateRepository>,
}

impl AppStateService {
    pub fn new(repository: Arc<dyn AppStateRepository>) -> Self {
        Self { repository }
    }

    pub async fn load(&self) -> anyhow::Result<AppStateSnapshot> {
        self.repository.load().await.map_err(Into::into)
    }

    pub async fn save(&self, state: &AppStateSnapshot) -> anyhow::Result<()> {
        self.repository.save(state).await.map_err(Into::into)
    }

    pub async fn load_entries_workspace(&self) -> anyhow::Result<EntriesWorkspaceState> {
        Ok(self.load().await?.entries_workspace)
    }

    pub async fn save_entries_workspace(
        &self,
        entries_workspace: EntriesWorkspaceState,
    ) -> anyhow::Result<()> {
        let mut state = self.load().await?;
        state.entries_workspace = entries_workspace;
        self.save(&state).await
    }

    pub async fn load_last_opened_feed_id(&self) -> anyhow::Result<Option<i64>> {
        Ok(self.load().await?.last_opened_feed_id)
    }

    pub async fn save_last_opened_feed_id(&self, feed_id: Option<i64>) -> anyhow::Result<()> {
        let mut state = self.load().await?;
        state.last_opened_feed_id = feed_id;
        self.save(&state).await
    }
}
