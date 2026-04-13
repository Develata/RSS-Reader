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

    async fn load_snapshot(&self) -> anyhow::Result<AppStateSnapshot> {
        self.repository.load().await.map_err(Into::into)
    }

    async fn save_snapshot(&self, state: &AppStateSnapshot) -> anyhow::Result<()> {
        self.repository.save(state).await.map_err(Into::into)
    }

    pub async fn load_entries_workspace(&self) -> anyhow::Result<EntriesWorkspaceState> {
        Ok(self.load_snapshot().await?.entries_workspace)
    }

    pub async fn save_entries_workspace(
        &self,
        entries_workspace: EntriesWorkspaceState,
    ) -> anyhow::Result<()> {
        let mut state = self.load_snapshot().await?;
        state.entries_workspace = entries_workspace;
        self.save_snapshot(&state).await
    }

    pub async fn load_last_opened_feed_id(&self) -> anyhow::Result<Option<i64>> {
        Ok(self.load_snapshot().await?.last_opened_feed_id)
    }

    pub async fn save_last_opened_feed_id(&self, feed_id: Option<i64>) -> anyhow::Result<()> {
        let mut state = self.load_snapshot().await?;
        state.last_opened_feed_id = feed_id;
        self.save_snapshot(&state).await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use rssr_domain::{AppStateRepository, AppStateSnapshot, EntriesWorkspaceState, ReadFilter};

    use super::AppStateService;

    struct AppStateRepositoryStub {
        state: Mutex<AppStateSnapshot>,
    }

    #[async_trait::async_trait]
    impl AppStateRepository for AppStateRepositoryStub {
        async fn load(&self) -> rssr_domain::Result<AppStateSnapshot> {
            Ok(self.state.lock().expect("lock app state").clone())
        }

        async fn save(&self, state: &AppStateSnapshot) -> rssr_domain::Result<()> {
            *self.state.lock().expect("lock app state") = state.clone();
            Ok(())
        }
    }

    fn service(state: AppStateSnapshot) -> AppStateService {
        AppStateService::new(Arc::new(AppStateRepositoryStub { state: Mutex::new(state) }))
    }

    #[tokio::test]
    async fn load_entries_workspace_returns_workspace_slice() {
        let workspace = EntriesWorkspaceState {
            read_filter: ReadFilter::UnreadOnly,
            ..EntriesWorkspaceState::default()
        };

        let loaded = service(AppStateSnapshot {
            entries_workspace: workspace.clone(),
            ..AppStateSnapshot::default()
        })
        .load_entries_workspace()
        .await
        .expect("load entries workspace");

        assert_eq!(loaded, workspace);
    }

    #[tokio::test]
    async fn save_entries_workspace_updates_only_workspace_slice() {
        let next = EntriesWorkspaceState {
            selected_feed_urls: vec!["https://example.com/feed.xml".to_string()],
            ..EntriesWorkspaceState::default()
        };
        let state =
            AppStateSnapshot { last_opened_feed_id: Some(7), ..AppStateSnapshot::default() };
        let service = service(state);

        service.save_entries_workspace(next.clone()).await.expect("save entries workspace");

        assert_eq!(service.load_entries_workspace().await.expect("load workspace"), next);
        assert_eq!(
            service.load_last_opened_feed_id().await.expect("load last opened feed id"),
            Some(7)
        );
    }

    #[tokio::test]
    async fn save_last_opened_feed_id_updates_only_last_opened_field() {
        let workspace =
            EntriesWorkspaceState { show_archived: true, ..EntriesWorkspaceState::default() };
        let service = service(AppStateSnapshot {
            entries_workspace: workspace.clone(),
            ..AppStateSnapshot::default()
        });

        service.save_last_opened_feed_id(Some(42)).await.expect("save last opened feed id");

        assert_eq!(
            service.load_last_opened_feed_id().await.expect("load last opened feed id"),
            Some(42)
        );
        assert_eq!(
            service.load_entries_workspace().await.expect("load entries workspace"),
            workspace
        );
    }
}
