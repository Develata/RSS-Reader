use rssr_domain::{ConfigFingerprint, DomainError, FingerprintFeed};

use crate::application_adapters::browser::state::BrowserState;

pub(super) fn map_persistence_error(error: impl std::fmt::Display) -> DomainError {
    DomainError::Persistence(error.to_string())
}

pub(super) fn map_store_error(error: anyhow::Error) -> DomainError {
    error
        .downcast::<DomainError>()
        .unwrap_or_else(|error| DomainError::Persistence(format!("{error:#}")))
}

/// The fingerprint of the browser's configuration: the same projection as the library's (raw
/// custom names, generations; a subscription without a recorded generation has generation 0).
pub(super) fn config_fingerprint(state: &BrowserState) -> ConfigFingerprint {
    let projection = state
        .core
        .feeds
        .iter()
        .filter(|feed| !feed.is_deleted)
        .map(|feed| FingerprintFeed {
            id: feed.id,
            url: &feed.url,
            generation: state.core.feed_generations.get(&feed.id).copied().unwrap_or(0),
            custom_title: feed.custom_title.as_deref(),
            folder: feed.folder.as_deref(),
        })
        .collect();
    ConfigFingerprint::compute(projection, &state.core.settings)
}
