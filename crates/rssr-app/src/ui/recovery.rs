//! What a failure asks of the whole app (phase C, C4b item 1): sign in again (`unauthenticated`)
//! or reload the page (`build_mismatch`). Every UI command runs through [`run_ui_command`], which
//! raises it; the app's root shows it and acts on it. The page that failed shows its own status
//! as well.

use dioxus::prelude::*;

use super::failure::Recovery;
use super::{UiCommand, UiIntent, execute_ui_command};

/// The app's recovery, once raised; signing in again outranks reloading. It is never cleared:
/// the page leaves (to sign in) or is reloaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum AppRecovery {
    /// The page is of another build than the server.
    Reload,
    /// The session ended.
    SignIn,
}

pub(crate) static APP_RECOVERY: GlobalSignal<Option<AppRecovery>> = Signal::global(|| None);

/// What `intents` ask of the whole app, if anything.
pub(crate) fn required(intents: &[UiIntent]) -> Option<AppRecovery> {
    intents
        .iter()
        .filter_map(UiIntent::failure)
        .filter_map(|failure| match failure.recovery() {
            Recovery::SignIn { .. } => Some(AppRecovery::SignIn),
            Recovery::Reload => Some(AppRecovery::Reload),
            Recovery::Failed | Recovery::Unknown => None,
        })
        .max()
}

/// Runs `command` and raises what its failures ask of the whole app. Every UI command goes
/// through here; [`execute_ui_command`] itself stays free of the app's state (its tests run
/// without one).
pub(crate) async fn run_ui_command(command: UiCommand) -> Vec<UiIntent> {
    let intents = execute_ui_command(command).await;
    if let Some(next) = required(&intents) {
        let current = *APP_RECOVERY.peek();
        if current.is_none_or(|current| next > current) {
            *APP_RECOVERY.write() = Some(next);
        }
    }
    intents
}

#[cfg(test)]
mod tests {
    use rssr_api::{BackendError, ErrorCode};

    use super::{AppRecovery, required};
    use crate::pages::entries_page::intent::EntriesPageIntent;
    use crate::ui::{UiIntent, failure::UiFailure};

    fn failed(code: ErrorCode, reason: Option<ErrorCode>) -> UiFailure {
        let error = BackendError::new(code, "原因");
        UiFailure::new(
            "刷新",
            if let Some(reason) = reason { error.with_reason(reason) } else { error },
        )
    }

    /// Signing in again and reloading come from failures wherever the intents carry them,
    /// including one no page shows; signing in again outranks reloading; other failures ask
    /// nothing of the app.
    #[test]
    fn the_app_recovers_from_sign_in_and_build_failures_only() {
        let unknown = failed(ErrorCode::OutcomeUnknown, None);
        let network = failed(ErrorCode::Network, None);
        assert_eq!(required(&[UiIntent::Failed(unknown), UiIntent::Failed(network)]), None);

        let stale = UiIntent::Unshown(failed(ErrorCode::BuildMismatch, None));
        assert_eq!(required(std::slice::from_ref(&stale)), Some(AppRecovery::Reload));

        let signed_out = UiIntent::EntriesPage(EntriesPageIntent::BulkFailed(failed(
            ErrorCode::OutcomeUnknown,
            Some(ErrorCode::Unauthenticated),
        )));
        assert_eq!(required(&[stale, signed_out]), Some(AppRecovery::SignIn));
    }
}
