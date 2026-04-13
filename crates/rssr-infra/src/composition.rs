use std::sync::Arc;

use rssr_application::{AppCompositionInput, AppUseCases};

#[cfg(not(target_arch = "wasm32"))]
use {
    crate::{
        application_adapters::{
            InfraFeedRefreshSource, InfraOpmlCodec, SqliteAppStateAdapter, SqliteRefreshStore,
        },
        db::{
            SqlitePool, app_state_repository::SqliteAppStateRepository,
            entry_repository::SqliteEntryRepository, feed_repository::SqliteFeedRepository,
            settings_repository::SqliteSettingsRepository,
        },
        fetch::FetchClient,
        opml::OpmlCodec,
        parser::FeedParser,
    },
    rssr_application::SystemClock,
};

#[cfg(target_arch = "wasm32")]
use {
    crate::application_adapters::browser::{
        adapters::{
            BrowserAppStateAdapter, BrowserEntryRepository, BrowserFeedRefreshSource,
            BrowserFeedRepository, BrowserOpmlCodec, BrowserRefreshStore,
            BrowserSettingsRepository,
        },
        state::BrowserState,
    },
    rssr_application::ClockPort,
    std::sync::Mutex,
};

#[cfg(not(target_arch = "wasm32"))]
pub struct NativeSqliteComposition {
    pub use_cases: AppUseCases,
    pub entry_repository: Arc<SqliteEntryRepository>,
}

#[cfg(not(target_arch = "wasm32"))]
pub fn compose_native_sqlite_use_cases(pool: SqlitePool) -> NativeSqliteComposition {
    let feed_repository = Arc::new(SqliteFeedRepository::new(pool.clone()));
    let entry_repository = Arc::new(SqliteEntryRepository::new(pool.clone()));
    let settings_repository = Arc::new(SqliteSettingsRepository::new(pool.clone()));
    let app_state_repository = Arc::new(SqliteAppStateRepository::new(pool));
    let app_state = Arc::new(SqliteAppStateAdapter::new(app_state_repository));

    let use_cases = AppUseCases::compose(AppCompositionInput {
        feed_repository: feed_repository.clone(),
        entry_repository: entry_repository.clone(),
        settings_repository,
        app_state,
        refresh_source: Arc::new(InfraFeedRefreshSource::new(
            FetchClient::new(),
            FeedParser::new(),
        )),
        refresh_store: Arc::new(SqliteRefreshStore::new(feed_repository, entry_repository.clone())),
        opml_codec: Arc::new(InfraOpmlCodec::new(OpmlCodec::new())),
        clock: Arc::new(SystemClock),
    });

    NativeSqliteComposition { use_cases, entry_repository }
}

#[cfg(target_arch = "wasm32")]
pub fn compose_browser_use_cases(
    state: Arc<Mutex<BrowserState>>,
    client: reqwest::Client,
    clock: Arc<dyn ClockPort>,
) -> AppUseCases {
    let feed_repository = Arc::new(BrowserFeedRepository::new(state.clone()));
    let entry_repository = Arc::new(BrowserEntryRepository::new(state.clone()));
    let settings_repository = Arc::new(BrowserSettingsRepository::new(state.clone()));
    let app_state = Arc::new(BrowserAppStateAdapter::new(state.clone()));

    AppUseCases::compose(AppCompositionInput {
        feed_repository,
        entry_repository,
        settings_repository,
        app_state,
        refresh_source: Arc::new(BrowserFeedRefreshSource::new(client.clone())),
        refresh_store: Arc::new(BrowserRefreshStore::new(state)),
        opml_codec: Arc::new(BrowserOpmlCodec),
        clock,
    })
}
