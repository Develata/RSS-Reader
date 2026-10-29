//! A failed backend call as the pages get it (phase C, C4b item 1): the error with its code and
//! what the user was doing. The runtime no longer turns the error into text; this is the one place
//! that reads the code and decides what it means for the page and for the app.

use rssr_api::{BackendError, ErrorCode};

/// What a failure asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Recovery {
    /// It did not happen: "……失败".
    Failed,
    /// It may have happened (`outcome_unknown`): say the outcome is unknown, not that it failed;
    /// release what is in progress, keep what the user typed, and read again what it may have
    /// changed (read-only).
    Unknown,
    /// The session ended: sign in again. `unknown` when the server had already accepted the
    /// operation (a job), so its outcome is unknown too.
    SignIn { unknown: bool },
    /// The page is of another build than the server: reload it. The write was refused before it
    /// ran.
    Reload,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UiFailure {
    /// What the user was doing, as it starts the message: "刷新", "保存设置".
    pub(crate) action: &'static str,
    pub(crate) error: BackendError,
}

impl UiFailure {
    pub(crate) fn new(action: &'static str, error: BackendError) -> Self {
        Self { action, error }
    }

    pub(crate) fn recovery(&self) -> Recovery {
        // The reason first: a job accepted before the session ended is unknown *and* needs a new
        // sign-in (design review C4D-R2 R2-01).
        match (self.error.code, self.error.reason) {
            (ErrorCode::OutcomeUnknown, Some(ErrorCode::Unauthenticated)) => {
                Recovery::SignIn { unknown: true }
            }
            (ErrorCode::OutcomeUnknown, _) => Recovery::Unknown,
            (ErrorCode::Unauthenticated, _) => Recovery::SignIn { unknown: false },
            (ErrorCode::BuildMismatch, _) => Recovery::Reload,
            _ => Recovery::Failed,
        }
    }

    /// A configuration import refused because the configuration changed since it was read.
    pub(crate) fn is_conflict(&self) -> bool {
        self.error.code == ErrorCode::Conflict
    }

    /// Whether the outcome is unknown (the operation may have taken effect).
    pub(crate) fn is_unknown(&self) -> bool {
        matches!(self.recovery(), Recovery::Unknown | Recovery::SignIn { unknown: true })
    }

    /// The status line. An unknown outcome is never called a failure; the error's own message
    /// says what is unknown and what to do.
    pub(crate) fn message(&self) -> String {
        let (action, detail) = (self.action, &self.error.message);
        if self.is_unknown() {
            format!("{action}：{detail}")
        } else {
            // "导入 OPML 失败": a space after a Latin word.
            let gap = if action.ends_with(|c: char| c.is_ascii_alphanumeric()) { " " } else { "" };
            format!("{action}{gap}失败：{detail}")
        }
    }

    /// The status line's tone: `unknown` for an unknown outcome, else `error`.
    pub(crate) fn tone(&self) -> &'static str {
        if self.is_unknown() { TONE_UNKNOWN } else { "error" }
    }
}

/// The tone of a status whose outcome is unknown (styled apart from an error).
pub(crate) const TONE_UNKNOWN: &str = "unknown";

/// Test only: a plain failure (`internal`) with `message`.
#[cfg(test)]
pub(crate) fn plain(message: &str) -> UiFailure {
    UiFailure::new("测试", BackendError::new(ErrorCode::Internal, message))
}

#[cfg(test)]
mod tests {
    use rssr_api::{BackendError, ErrorCode};

    use super::{Recovery, UiFailure};

    fn failure(code: ErrorCode, reason: Option<ErrorCode>) -> UiFailure {
        let error = BackendError::new(code, "原因");
        UiFailure::new(
            "保存设置",
            match reason {
                Some(reason) => error.with_reason(reason),
                None => error,
            },
        )
    }

    /// Each class of error means one thing, the reason before the code; an unknown outcome is
    /// never called a failure.
    #[test]
    fn every_error_has_one_meaning() {
        let cases = [
            (ErrorCode::Network, None, Recovery::Failed, "保存设置失败：原因", "error"),
            (ErrorCode::Conflict, None, Recovery::Failed, "保存设置失败：原因", "error"),
            (ErrorCode::OutcomeUnknown, None, Recovery::Unknown, "保存设置：原因", "unknown"),
            (
                ErrorCode::OutcomeUnknown,
                Some(ErrorCode::Unauthenticated),
                Recovery::SignIn { unknown: true },
                "保存设置：原因",
                "unknown",
            ),
            (
                ErrorCode::Unauthenticated,
                None,
                Recovery::SignIn { unknown: false },
                "保存设置失败：原因",
                "error",
            ),
            (ErrorCode::BuildMismatch, None, Recovery::Reload, "保存设置失败：原因", "error"),
        ];
        for (code, reason, recovery, message, tone) in cases {
            let failure = failure(code, reason);
            assert_eq!(failure.recovery(), recovery, "{code:?} {reason:?}");
            assert_eq!(failure.message(), message, "{code:?} {reason:?}");
            assert_eq!(failure.tone(), tone, "{code:?} {reason:?}");
        }
        assert!(failure(ErrorCode::Conflict, None).is_conflict());
        let opml = UiFailure::new("导入 OPML", BackendError::new(ErrorCode::InvalidInput, "原因"));
        assert_eq!(opml.message(), "导入 OPML 失败：原因");
    }
}
