use std::sync::Arc;

use rssr_domain::{AppStateRepository, EntryRepository, FeedRepository, SettingsRepository};

use crate::{
    AppStatePort, AppStateService, ClockPort, EntriesListService, EntriesWorkspaceService,
    EntryService, FeedCatalogService, FeedRefreshSourcePort, FeedRemovalCleanupPort, FeedService,
    FeedsSnapshotService, ImportExportService, OpmlCodecPort, ReaderService, RefreshService,
    RefreshStorePort, SettingsPageService, SettingsService, SettingsSyncService, ShellService,
    StartupService, SubscriptionWorkflow,
};

pub trait AppStateServicesPort:
    AppStateRepository + AppStatePort + FeedRemovalCleanupPort + Send + Sync
{
}

impl<T> AppStateServicesPort for T where
    T: AppStateRepository + AppStatePort + FeedRemovalCleanupPort + Send + Sync + ?Sized
{
}

pub struct AppCompositionInput {
    pub feed_repository: Arc<dyn FeedRepository>,
    pub entry_repository: Arc<dyn EntryRepository>,
    pub settings_repository: Arc<dyn SettingsRepository>,
    pub app_state: Arc<dyn AppStateServicesPort>,
    pub refresh_source: Arc<dyn FeedRefreshSourcePort>,
    pub refresh_store: Arc<dyn RefreshStorePort>,
    pub opml_codec: Arc<dyn OpmlCodecPort>,
    pub clock: Arc<dyn ClockPort>,
}

#[derive(Clone)]
pub struct AppUseCases {
    pub feed_catalog_service: FeedCatalogService,
    pub feed_service: FeedService,
    pub entry_service: EntryService,
    pub settings_service: SettingsService,
    pub settings_sync_service: SettingsSyncService,
    pub settings_page_service: SettingsPageService,
    pub shell_service: ShellService,
    pub app_state_service: AppStateService,
    pub refresh_service: RefreshService,
    pub subscription_workflow: SubscriptionWorkflow,
    pub import_export_service: ImportExportService,
    pub startup_service: StartupService,
    pub entries_list_service: EntriesListService,
    pub entries_workspace_service: EntriesWorkspaceService,
    pub feeds_snapshot_service: FeedsSnapshotService,
    pub reader_service: ReaderService,
}

impl AppUseCases {
    pub fn compose(input: AppCompositionInput) -> Self {
        let feed_service =
            FeedService::new(input.feed_repository.clone(), input.entry_repository.clone());
        let feed_catalog_service = FeedCatalogService::new(input.feed_repository.clone());
        let refresh_service = RefreshService::new(input.refresh_source, input.refresh_store);

        let settings_service = SettingsService::new(input.settings_repository.clone());
        let settings_sync_service = SettingsSyncService::new(settings_service.clone());
        let settings_page_service =
            SettingsPageService::new(settings_service.clone(), settings_sync_service.clone());
        let shell_service = ShellService::new(settings_service.clone());
        let app_state_service = AppStateService::new(input.app_state.clone());
        let entry_service = EntryService::new(input.entry_repository.clone());

        Self {
            feed_catalog_service,
            feed_service: feed_service.clone(),
            entry_service: entry_service.clone(),
            settings_service: settings_service.clone(),
            settings_sync_service,
            settings_page_service,
            shell_service,
            app_state_service: app_state_service.clone(),
            refresh_service: refresh_service.clone(),
            subscription_workflow: SubscriptionWorkflow::new(
                feed_service.clone(),
                refresh_service,
                input.app_state.clone(),
            ),
            import_export_service: ImportExportService::new_with_feed_removal_cleanup_and_clock(
                input.feed_repository.clone(),
                input.entry_repository,
                input.settings_repository,
                input.opml_codec,
                input.app_state,
                input.clock,
            ),
            startup_service: StartupService::new(
                settings_service.clone(),
                app_state_service.clone(),
                input.feed_repository.clone(),
            ),
            entries_list_service: EntriesListService::new(entry_service.clone()),
            entries_workspace_service: EntriesWorkspaceService::new(
                settings_service,
                app_state_service,
                input.feed_repository.clone(),
            ),
            feeds_snapshot_service: FeedsSnapshotService::new(input.feed_repository),
            reader_service: ReaderService::new(entry_service),
        }
    }
}
