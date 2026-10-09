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

    pub async fn load_entries_workspace(&self) -> anyhow::Result<EntriesWorkspaceState> {
        Ok(self.load_snapshot().await?.entries_workspace)
    }

    /// 只改文章页偏好；读改写由仓储原子完成，不会覆盖并发写入的其他字段（审计 A2）。
    pub async fn save_entries_workspace(
        &self,
        entries_workspace: EntriesWorkspaceState,
    ) -> anyhow::Result<()> {
        self.repository
            .update(Box::new(move |state| state.entries_workspace = entries_workspace))
            .await
            .map_err(Into::into)
    }

    pub async fn load_last_opened_feed_id(&self) -> anyhow::Result<Option<i64>> {
        Ok(self.load_snapshot().await?.last_opened_feed_id)
    }

    pub async fn save_last_opened_feed_id(&self, feed_id: Option<i64>) -> anyhow::Result<()> {
        self.repository
            .update(Box::new(move |state| state.last_opened_feed_id = feed_id))
            .await
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use rssr_domain::{
        AppStateMutation, AppStateRepository, AppStateSnapshot, EntriesWorkspaceState, ReadFilter,
    };

    use super::AppStateService;

    struct AppStateRepositoryStub {
        state: Mutex<AppStateSnapshot>,
    }

    #[async_trait::async_trait]
    impl AppStateRepository for AppStateRepositoryStub {
        async fn load(&self) -> rssr_domain::Result<AppStateSnapshot> {
            Ok(self.state.lock().expect("lock app state").clone())
        }

        async fn update(&self, mutate: AppStateMutation) -> rssr_domain::Result<()> {
            mutate(&mut self.state.lock().expect("lock app state"));
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

    /// 窄字段更新只能通过仓储的原子 `update` 完成（审计 A2）。读整份再写回的实现会在两个
    /// 并发更新之间丢掉一方的修改；这个仓储在 `update` 之外的读取一律判为失败，并让两个更新
    /// 在各自的修改应用前交替让出执行权。
    struct AtomicOnlyRepository {
        state: Mutex<AppStateSnapshot>,
        loads: std::sync::atomic::AtomicUsize,
    }

    #[async_trait::async_trait]
    impl AppStateRepository for AtomicOnlyRepository {
        async fn load(&self) -> rssr_domain::Result<AppStateSnapshot> {
            self.loads.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(self.state.lock().expect("lock app state").clone())
        }

        async fn update(&self, mutate: AppStateMutation) -> rssr_domain::Result<()> {
            tokio::task::yield_now().await;
            mutate(&mut self.state.lock().expect("lock app state"));
            Ok(())
        }
    }

    #[tokio::test]
    async fn concurrent_narrow_field_updates_do_not_lose_each_other() {
        let repository = Arc::new(AtomicOnlyRepository {
            state: Mutex::new(AppStateSnapshot::default()),
            loads: std::sync::atomic::AtomicUsize::new(0),
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
        assert_eq!(
            repository.loads.load(std::sync::atomic::Ordering::SeqCst),
            0,
            "narrow updates must not read the snapshot outside the atomic update"
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
