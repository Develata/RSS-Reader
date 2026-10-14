//! The error every [`crate::Backend`] operation returns: a stable code, a message for the
//! user, and a request id to match server logs (`docs/design/v0.2-phase-b2-contracts.md`, D5).

use std::fmt;

use rssr_application::{ErrorKind, classify};

/// Stable error codes. Phase C adds protocol-level ones (authentication, version mismatch).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorCode {
    InvalidInput,
    NotFound,
    /// The data changed since it was read; read again before retrying.
    Conflict,
    Network,
    /// Busy or timed out; retrying later may work.
    Unavailable,
    Storage,
    Internal,
}

impl ErrorCode {
    /// The code as it appears on the wire.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InvalidInput => "invalid_input",
            Self::NotFound => "not_found",
            Self::Conflict => "conflict",
            Self::Network => "network",
            Self::Unavailable => "unavailable",
            Self::Storage => "storage",
            Self::Internal => "internal",
        }
    }
}

impl From<ErrorKind> for ErrorCode {
    fn from(kind: ErrorKind) -> Self {
        match kind {
            ErrorKind::InvalidInput => Self::InvalidInput,
            ErrorKind::NotFound => Self::NotFound,
            ErrorKind::Conflict => Self::Conflict,
            ErrorKind::Network => Self::Network,
            ErrorKind::Unavailable => Self::Unavailable,
            ErrorKind::Storage => Self::Storage,
            ErrorKind::Internal => Self::Internal,
        }
    }
}

/// A failed operation as the user interface sees it. Its `Display` is the message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendError {
    pub code: ErrorCode,
    /// For the user; internal details stay in the log.
    pub message: String,
    /// Matches the server's log entry; `None` for an in-process backend.
    pub request_id: Option<String>,
}

impl fmt::Display for BackendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for BackendError {}

/// Classified once, where the error leaves the use cases (application's `classify`, which also
/// logs what it does not show).
impl From<anyhow::Error> for BackendError {
    fn from(error: anyhow::Error) -> Self {
        let public = classify(&error);
        Self { code: public.kind.into(), message: public.message, request_id: None }
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Context;
    use rssr_domain::DomainError;

    use super::{BackendError, ErrorCode};

    #[test]
    fn an_error_keeps_its_code_through_contexts() {
        let error: BackendError = Err::<(), _>(DomainError::Conflict("配置已被修改".into()))
            .context("替换配置失败")
            .unwrap_err()
            .into();
        assert_eq!(error.code, ErrorCode::Conflict);
        assert_eq!(error.to_string(), "配置已被修改");
        assert_eq!(error.code.as_str(), "conflict");
        assert_eq!(error.request_id, None);
    }
}
