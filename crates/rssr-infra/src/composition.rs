use std::sync::Arc;

use crate::{application_adapters::InfraOpmlCodec, opml::OpmlCodec};
use rssr_application::{AppCompositionInput, AppUseCases};

#[cfg(not(target_arch = "wasm32"))]
use crate::{
    application_adapters::InfraFeedRefreshSource, fetch::FetchClient, outbound::OutboundPolicy,
    parser::FeedParser, subscription_probe::HttpSubscriptionProbe,
};

/// Use cases over one library, plus the entry store used by body localization.
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
    if *policy == OutboundPolicy::Unrestricted {
        return Ok(compose_library_use_cases(store));
    }
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
    compose_library_ports(
        store,
        Arc::new(subscription_probe),
        Arc::new(InfraFeedRefreshSource::new(fetch_client, FeedParser::new())),
    )
}

/// A library whose network ports are unavailable, including when called directly through use
/// cases. The Pages host additionally disables the periodic refresh loop and remote config.
pub fn compose_offline_library_use_cases(store: crate::store::LibraryStore) -> LibraryComposition {
    compose_library_ports(store, Arc::new(NetworkDisabled), Arc::new(NetworkDisabled))
}

fn compose_library_ports(
    store: crate::store::LibraryStore,
    subscription_probe: Arc<dyn rssr_application::SubscriptionProbePort>,
    refresh_source: Arc<dyn rssr_application::FeedRefreshSourcePort>,
) -> LibraryComposition {
    use crate::store::{
        LibraryAppState, LibraryEntries, LibraryFeeds, LibraryMutations, LibraryQueries,
        LibraryRefreshStore, LibrarySettings,
    };

    let entries = Arc::new(LibraryEntries::new(store.clone()));
    let mutations = Arc::new(LibraryMutations::new(store.clone()));
    let settings = Arc::new(LibrarySettings::new(store.clone()));
    let workspace_edits = Arc::new(crate::store::LibraryWorkspaceEdits::new(store.clone()));
    let use_cases = AppUseCases::compose(AppCompositionInput {
        entry_batch: workspace_edits.clone(),
        feed_edit: workspace_edits,
        feed_repository: Arc::new(LibraryFeeds::new(store.clone())),
        entry_index_repository: entries.clone(),
        settings_repository: settings.clone(),
        settings_update: settings,
        app_state: Arc::new(LibraryAppState::new(store.clone())),
        subscription_probe,
        refresh_source,
        refresh_store: Arc::new(LibraryRefreshStore::new(store.clone())),
        opml_codec: Arc::new(InfraOpmlCodec::new(OpmlCodec::new())),
        clock: Arc::new(crate::platform_clock::PlatformClock),
        subscription_activation: mutations.clone(),
        subscription_removal: mutations.clone(),
        config_replacement: mutations,
        library_queries: Arc::new(LibraryQueries::new(store)),
    });
    LibraryComposition { use_cases, entries }
}

struct NetworkDisabled;

fn network_disabled() -> anyhow::Error {
    rssr_domain::DomainError::Network("静态演示已禁用在线订阅与刷新。".into()).into()
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl rssr_application::SubscriptionProbePort for NetworkDisabled {
    async fn probe(
        &self,
        _url: &url::Url,
    ) -> anyhow::Result<rssr_application::SubscriptionProbeOutcome> {
        Err(network_disabled())
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl rssr_application::FeedRefreshSourcePort for NetworkDisabled {
    async fn refresh(
        &self,
        _target: &rssr_application::RefreshTarget,
    ) -> anyhow::Result<rssr_application::FeedRefreshSourceOutput> {
        Err(network_disabled())
    }
}
