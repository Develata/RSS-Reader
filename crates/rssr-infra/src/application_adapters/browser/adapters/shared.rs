use rssr_domain::DomainError;

pub(super) fn map_persistence_error(error: impl std::fmt::Display) -> DomainError {
    DomainError::Persistence(error.to_string())
}

pub(super) fn map_store_error(error: anyhow::Error) -> DomainError {
    error
        .downcast::<DomainError>()
        .unwrap_or_else(|error| DomainError::Persistence(format!("{error:#}")))
}
