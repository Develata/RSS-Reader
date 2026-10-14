use std::sync::Arc;

use rssr_domain::{AppStateRepository, EntryIndexRepository, FeedRepository, SettingsRepository};

use crate::{
    AppStatePort, AppStateService, ClockPort, ConfigReplacementPort, EntriesListService,
    EntriesWorkspaceService, EntryFlagsService, FeedCatalogService, FeedRefreshSourcePort,
    FeedService, FeedsSnapshotService, ImportExportService, LibraryQueryPort, OpmlCodecPort,
    ReaderService, RefreshService, RefreshStorePort, SettingsService, SettingsSyncService,
    StartupService, SubscriptionActivationPort, SubscriptionRemovalPort, SubscriptionWorkflow,
};

// 架构护栏：如果某次设计/计划开始要求严重代码分叉、污染 infra 边界、引发前后端大规模迁移
// （纯前端内部或纯后端内部重构除外），或明显违背设计哲学，必须先停下来做保守分析并
// 明确向交互人员提出风险；不要把这类结构性代价静默推进到 application 组合层。
pub trait AppStateServicesPort: AppStateRepository + AppStatePort + Send + Sync {}

impl<T> AppStateServicesPort for T where T: AppStateRepository + AppStatePort + Send + Sync + ?Sized {}

pub struct AppCompositionInput {
    pub feed_repository: Arc<dyn FeedRepository>,
    pub entry_index_repository: Arc<dyn EntryIndexRepository>,
    pub settings_repository: Arc<dyn SettingsRepository>,
    pub app_state: Arc<dyn AppStateServicesPort>,
    pub subscription_probe: Arc<dyn crate::SubscriptionProbePort>,
    pub refresh_source: Arc<dyn FeedRefreshSourcePort>,
    pub refresh_store: Arc<dyn RefreshStorePort>,
    pub opml_codec: Arc<dyn OpmlCodecPort>,
    pub clock: Arc<dyn ClockPort>,
    pub subscription_activation: Arc<dyn SubscriptionActivationPort>,
    pub subscription_removal: Arc<dyn SubscriptionRemovalPort>,
    pub config_replacement: Arc<dyn ConfigReplacementPort>,
    /// Reads the user interface needs, each answered from one snapshot of the library.
    pub library_queries: Arc<dyn LibraryQueryPort>,
}

#[derive(Clone)]
pub struct AppUseCases {
    pub feed_catalog_service: FeedCatalogService,
    pub feed_service: FeedService,
    pub settings_service: SettingsService,
    pub settings_sync_service: SettingsSyncService,
    pub app_state_service: AppStateService,
    pub refresh_service: RefreshService,
    pub subscription_workflow: SubscriptionWorkflow,
    pub import_export_service: ImportExportService,
    pub startup_service: StartupService,
    pub entries_list_service: EntriesListService,
    pub entries_workspace_service: EntriesWorkspaceService,
    pub entry_flags_service: EntryFlagsService,
    pub feeds_snapshot_service: FeedsSnapshotService,
    pub reader_service: ReaderService,
}

impl AppUseCases {
    pub fn compose(input: AppCompositionInput) -> Self {
        let feed_service = FeedService::new(
            input.feed_repository.clone(),
            input.subscription_activation,
            input.subscription_removal,
        );
        let feed_catalog_service = FeedCatalogService::new(input.feed_repository.clone());
        let refresh_service = RefreshService::new(input.refresh_source, input.refresh_store);

        let settings_service = SettingsService::new(input.settings_repository);
        let settings_sync_service = SettingsSyncService::new(settings_service.clone());
        let app_state_service = AppStateService::new(input.app_state.clone());

        Self {
            feed_catalog_service,
            feed_service: feed_service.clone(),
            settings_service: settings_service.clone(),
            settings_sync_service,
            app_state_service: app_state_service.clone(),
            refresh_service: refresh_service.clone(),
            subscription_workflow: SubscriptionWorkflow::new(
                feed_service.clone(),
                refresh_service,
                input.subscription_probe,
            ),
            import_export_service: ImportExportService::new(
                input.feed_repository.clone(),
                input.library_queries.clone(),
                input.opml_codec,
                input.clock,
                input.config_replacement,
            ),
            startup_service: StartupService::new(input.library_queries.clone()),
            entries_list_service: EntriesListService::new(
                input.entry_index_repository.clone(),
                input.library_queries.clone(),
            ),
            entry_flags_service: EntryFlagsService::new(input.entry_index_repository),
            entries_workspace_service: EntriesWorkspaceService::new(
                app_state_service,
                input.library_queries.clone(),
            ),
            feeds_snapshot_service: FeedsSnapshotService::new(input.feed_repository),
            reader_service: ReaderService::new(input.library_queries),
        }
    }
}
