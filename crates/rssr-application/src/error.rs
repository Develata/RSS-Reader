//! What an error means to the user, decided before it becomes text
//! (`docs/design/v0.2-phase-b2-contracts.md`, D5).
//!
//! Use cases keep returning `anyhow` errors; the kind comes from the first [`DomainError`] in the
//! error chain, so a context added on the way up never changes it. An error without one is
//! internal. Only kinds that describe the request or the remote side show their own text; the
//! others show a fixed explanation and leave the full chain to the log.

use rssr_domain::DomainError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorKind {
    /// The request itself is wrong (a bad address, an invalid setting, a malformed file).
    InvalidInput,
    NotFound,
    /// The data changed since the caller read it; read again before retrying.
    Conflict,
    /// A remote site could not be reached or answered with a failure.
    Network,
    /// The library is busy or timed out; retrying later may work.
    Unavailable,
    /// Reading or writing local data failed.
    Storage,
    /// Anything else: a defect or an unexpected failure.
    Internal,
}

impl ErrorKind {
    /// Whether the error's own text is meant for the user; otherwise [`Self::generic_message`].
    pub fn shows_details(self) -> bool {
        matches!(self, Self::InvalidInput | Self::NotFound | Self::Conflict | Self::Network)
    }

    pub fn generic_message(self) -> &'static str {
        match self {
            Self::InvalidInput => "请求无效",
            Self::NotFound => "未找到请求的资源",
            Self::Conflict => "数据已被修改，请重新读取后再试",
            Self::Network => "无法访问远端",
            Self::Unavailable => "资料库正忙，请稍后重试",
            Self::Storage => "本地数据读写失败，请稍后重试",
            Self::Internal => "发生内部错误，请稍后重试",
        }
    }
}

/// An error as the user may see it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicError {
    pub kind: ErrorKind,
    pub message: String,
}

fn kind_of(error: &DomainError) -> ErrorKind {
    match error {
        DomainError::InvalidInput(_) => ErrorKind::InvalidInput,
        DomainError::NotFound => ErrorKind::NotFound,
        DomainError::Conflict(_) => ErrorKind::Conflict,
        DomainError::Network(_) => ErrorKind::Network,
        DomainError::Unavailable(_) => ErrorKind::Unavailable,
        DomainError::Persistence(_) => ErrorKind::Storage,
    }
}

/// The detail a domain error carries, without the category prefix of its `Display`.
fn detail_of(error: &DomainError) -> Option<&str> {
    match error {
        DomainError::InvalidInput(detail)
        | DomainError::Conflict(detail)
        | DomainError::Network(detail) => Some(detail),
        DomainError::NotFound | DomainError::Unavailable(_) | DomainError::Persistence(_) => None,
    }
}

/// The kind and the user-facing text of `error`. Kinds that do not show their details are
/// logged here with the whole chain, the only place it is kept.
pub fn classify(error: &anyhow::Error) -> PublicError {
    let domain = error.chain().find_map(|cause| cause.downcast_ref::<DomainError>());
    let kind = domain.map_or(ErrorKind::Internal, kind_of);
    let message = match domain.and_then(detail_of) {
        Some(detail) if kind.shows_details() => detail.to_owned(),
        _ => {
            if !kind.shows_details() {
                tracing::warn!(?kind, error = %format!("{error:#}"), "操作失败");
            }
            kind.generic_message().to_owned()
        }
    };
    PublicError { kind, message }
}

#[cfg(test)]
mod tests {
    use anyhow::Context;
    use rssr_domain::DomainError;

    use super::{ErrorKind, classify};

    #[test]
    fn the_kind_survives_contexts_and_only_public_kinds_show_their_text() {
        let wrapped = |error: DomainError| {
            Err::<(), _>(error).context("保存设置失败").context("外层").unwrap_err()
        };
        let cases = [
            (
                DomainError::InvalidInput("每页数量超出范围".into()),
                ErrorKind::InvalidInput,
                "每页数量超出范围",
            ),
            (DomainError::NotFound, ErrorKind::NotFound, "未找到请求的资源"),
            (DomainError::Conflict("配置已被修改".into()), ErrorKind::Conflict, "配置已被修改"),
            (DomainError::Network("HTTP 404".into()), ErrorKind::Network, "HTTP 404"),
            (
                DomainError::Unavailable("database is locked".into()),
                ErrorKind::Unavailable,
                "资料库正忙，请稍后重试",
            ),
            (
                DomainError::Persistence("disk I/O error".into()),
                ErrorKind::Storage,
                "本地数据读写失败，请稍后重试",
            ),
        ];
        for (error, kind, message) in cases {
            let public = classify(&wrapped(error));
            assert_eq!((public.kind, public.message.as_str()), (kind, message));
        }
        let untyped = classify(&anyhow::anyhow!("serde: expected value").context("导入失败"));
        assert_eq!(untyped.kind, ErrorKind::Internal);
        assert_eq!(untyped.message, "发生内部错误，请稍后重试");
    }
}
