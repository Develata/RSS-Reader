//! Failures of requests to remote sites, typed for the user (B2-D5): a request the user can
//! correct is invalid input, everything the remote side or the way to it caused is network.
//! The text keeps the whole chain, which names the address, the status or the reason.

use rssr_domain::DomainError;

pub(crate) fn invalid(error: anyhow::Error) -> anyhow::Error {
    DomainError::InvalidInput(format!("{error:#}")).into()
}

pub(crate) fn network(error: anyhow::Error) -> anyhow::Error {
    DomainError::Network(format!("{error:#}")).into()
}
