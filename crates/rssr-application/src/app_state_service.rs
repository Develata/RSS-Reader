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

    /// 两个窄字段更新并发时，各自的“读整份→改一个字段→写整份”会互相覆盖。
    /// 仓储在 `load` 后等待另一方也完成读取，确定性地构造这一交错（不依赖时序）。
    /// 这个夹具对应当前的 load/save 端口；窄字段更新改成原子端口操作后，应换成新端口的夹具。
    struct InterleavingRepository {
        state: Mutex<AppStateSnapshot>,
        both_loaded: tokio::sync::Barrier,
    }

    #[async_trait::async_trait]
    impl AppStateRepository for InterleavingRepository {
        async fn load(&self) -> rssr_domain::Result<AppStateSnapshot> {
            let snapshot = self.state.lock().expect("lock app state").clone();
            // 超时即失败：交错没有建立时，测试不能静默放行。
            tokio::time::timeout(std::time::Duration::from_secs(5), self.both_loaded.wait())
                .await
                .expect("both read-modify-write cycles must read before either saves");
            Ok(snapshot)
        }

        async fn save(&self, state: &AppStateSnapshot) -> rssr_domain::Result<()> {
            *self.state.lock().expect("lock app state") = state.clone();
            Ok(())
        }
    }

    #[tokio::test]
    #[ignore = "audit-2026-10-09: AppState narrow-field updates are whole-snapshot read-modify-write; concurrent updates lose one field (docs/engineering-audit-2026-10.md)"]
    async fn concurrent_narrow_field_updates_do_not_lose_each_other() {
        let repository = Arc::new(InterleavingRepository {
            state: Mutex::new(AppStateSnapshot::default()),
            both_loaded: tokio::sync::Barrier::new(2),
        });
        let service = AppStateService::new(repository.clone());
        let workspace =
            EntriesWorkspaceState { show_archived: true, ..EntriesWorkspaceState::default() };

        let (feed_saved, workspace_saved) = tokio::join!(
            service.save_last_opened_feed_id(Some(7)),
            service.save_entries_workspace(workspace.clone()),
        );
        feed_saved.expect("save last opened feed");
        workspace_saved.expect("save workspace");

        let persisted = repository.state.lock().expect("lock app state").clone();
        assert_eq!(persisted.last_opened_feed_id, Some(7), "last opened feed was lost");
        assert_eq!(persisted.entries_workspace, workspace, "workspace update was lost");
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
