use rssr_domain::DomainError;

pub(super) fn map_persistence_error(error: impl std::fmt::Display) -> DomainError {
    DomainError::Persistence(error.to_string())
}
