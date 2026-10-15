use std::sync::Arc;

use rssr_application::{AppCompositionInput, AppUseCases};

#[cfg(not(target_arch = "wasm32"))]
use {
    crate::{
        application_adapters::{InfraFeedRefreshSource, InfraOpmlCodec},
        fetch::FetchClient,
        opml::OpmlCodec,
        outbound::OutboundPolicy,
        parser::FeedParser,
        subscription_probe::HttpSubscriptionProbe,
    },
    rssr_application::SystemClock,
};

#[cfg(target_arch = "wasm32")]
use {
    crate::application_adapters::browser::{
        adapters::{
            BrowserAppStateAdapter, BrowserEntryRepository, BrowserFeedRefreshSource,
            BrowserFeedRepository, BrowserLibraryQueries, BrowserOpmlCodec,
            BrowserPersistenceMutations, BrowserRefreshStore, BrowserSettingsRepository,
        },
        state::BrowserStore,
    },
    rssr_application::ClockPort,
};

/// Use cases over one v0.2 library file, plus the entry store the GUI's body localization uses.
#[cfg(not(target_arch = "wasm32"))]
pub struct LibraryComposition {
    pub use_cases: AppUseCases,
    pub entries: Arc<crate::store::LibraryEntries>,
}

#[cfg(not(target_arch = "wasm32"))]
pub fn compose_library_use_cases(store: crate::store::LibraryStore) -> LibraryComposition {
    compose_library_with_sources(
        store,
        HttpSubscriptionProbe::with_feed_client(),
        FetchClient::new(),
    )
}

/// [`compose_library_use_cases`] whose subscription probing and feed fetching follow `policy`
/// (a server's [`OutboundPolicy::PublicOnly`]); a client that cannot be built is an error.
#[cfg(not(target_arch = "wasm32"))]
pub fn compose_library_use_cases_with(
    store: crate::store::LibraryStore,
    policy: &OutboundPolicy,
) -> anyhow::Result<LibraryComposition> {
    Ok(compose_library_with_sources(
        store,
        HttpSubscriptionProbe::with_policy(policy.clone())?,
        FetchClient::with_policy(policy.clone())?,
    ))
}

#[cfg(not(target_arch = "wasm32"))]
fn compose_library_with_sources(
    store: crate::store::LibraryStore,
    subscription_probe: HttpSubscriptionProbe,
    fetch_client: FetchClient,
) -> LibraryComposition {
    use crate::store::{
        LibraryAppState, LibraryEntries, LibraryFeeds, LibraryMutations, LibraryQueries,
        LibraryRefreshStore, LibrarySettings,
    };

    let entries = Arc::new(LibraryEntries::new(store.clone()));
    let mutations = Arc::new(LibraryMutations::new(store.clone()));
    let settings = Arc::new(LibrarySettings::new(store.clone()));
    let use_cases = AppUseCases::compose(AppCompositionInput {
        feed_repository: Arc::new(LibraryFeeds::new(store.clone())),
        entry_index_repository: entries.clone(),
        settings_repository: settings.clone(),
        settings_update: settings,
        app_state: Arc::new(LibraryAppState::new(store.clone())),
        subscription_probe: Arc::new(subscription_probe),
        refresh_source: Arc::new(InfraFeedRefreshSource::new(fetch_client, FeedParser::new())),
        refresh_store: Arc::new(LibraryRefreshStore::new(store.clone())),
        opml_codec: Arc::new(InfraOpmlCodec::new(OpmlCodec::new())),
        clock: Arc::new(SystemClock),
        subscription_activation: mutations.clone(),
        subscription_removal: mutations.clone(),
        config_replacement: mutations,
        library_queries: Arc::new(LibraryQueries::new(store)),
    });
    LibraryComposition { use_cases, entries }
}

#[cfg(target_arch = "wasm32")]
pub fn compose_browser_use_cases(
    state: BrowserStore,
    client: reqwest::Client,
    clock: Arc<dyn ClockPort>,
) -> AppUseCases {
    let feed_repository = Arc::new(BrowserFeedRepository::new(state.clone()));
    let entry_repository = Arc::new(BrowserEntryRepository::new(state.clone()));
    let settings_repository = Arc::new(BrowserSettingsRepository::new(state.clone()));
    let app_state = Arc::new(BrowserAppStateAdapter::new(state.clone()));
    let persistence_mutations = Arc::new(BrowserPersistenceMutations::new(state.clone()));

    AppUseCases::compose(AppCompositionInput {
        feed_repository: feed_repository.clone(),
        entry_index_repository: entry_repository,
        settings_repository: settings_repository.clone(),
        settings_update: settings_repository,
        app_state,
        subscription_probe: Arc::new(crate::subscription_probe::HttpSubscriptionProbe::new(
            client.clone(),
        )),
        refresh_source: Arc::new(BrowserFeedRefreshSource::new(client.clone())),
        refresh_store: Arc::new(BrowserRefreshStore::new(state.clone())),
        opml_codec: Arc::new(BrowserOpmlCodec),
        clock,
        subscription_activation: persistence_mutations.clone(),
        subscription_removal: persistence_mutations.clone(),
        config_replacement: persistence_mutations,
        library_queries: Arc::new(BrowserLibraryQueries::new(state)),
    })
}
