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
///
/// An unknown outcome may have changed the library: the pages re-read what they show (C4b
/// item 2), through the shell when there is one.
pub(crate) async fn run_ui_command(command: UiCommand) -> Vec<UiIntent> {
    let intents = execute_ui_command(command).await;
    if let Some(next) = required(&intents) {
        let current = *APP_RECOVERY.peek();
        if current.is_none_or(|current| next > current) {
            *APP_RECOVERY.write() = Some(next);
        }
    }
    if intents.iter().filter_map(UiIntent::failure).any(|failure| failure.is_unknown())
        && let Some(shell) = try_consume_context::<super::AppShellState>()
    {
        shell.request_reread();
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

/// [`run_ui_command`] itself raises the recovery, through the real runtime and a Dioxus runtime
/// (review C4B-A-02): a reload from a failure no page shows, then signing in again over it, which
/// a later reload does not lower; a plain failure raises nothing.
#[cfg(all(test, not(target_arch = "wasm32")))]
mod wrapper_tests {
    use std::time::Duration;

    use dioxus::prelude::*;
    use rssr_api::{BackendError, ErrorCode};

    use super::{APP_RECOVERY, AppRecovery, run_ui_command};
    use crate::ui::commands::{ReaderCommand, ShellCommand, UiCommand};
    use crate::ui::runtime::{backend::test_host, fake_backend::Scripted};

    fn app() -> Element {
        rsx! {}
    }

    /// Runs `command` through the wrapper in `dom` and gives the app's recovery after it.
    async fn raised(dom: &mut VirtualDom, command: UiCommand) -> Option<AppRecovery> {
        let done = dom.in_scope(ScopeId::APP, || {
            let mut done = Signal::new(false);
            spawn(async move {
                run_ui_command(command).await;
                done.set(true);
            });
            done
        });
        for _ in 0..200 {
            let _ = tokio::time::timeout(Duration::from_millis(20), dom.wait_for_work()).await;
            if dom.in_scope(ScopeId::APP, || *done.peek()) {
                return dom.in_scope(ScopeId::APP, || *APP_RECOVERY.peek());
            }
        }
        panic!("the command did not end");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn the_wrapper_raises_what_failures_ask_of_the_app() {
        let backend = Scripted::over_new_library();
        let _host = test_host::install(backend.clone());
        let mut dom = VirtualDom::new(app);
        dom.rebuild_in_place();

        backend.fail("refresh_all", BackendError::new(ErrorCode::Network, "网络中断"));
        let refresh = || UiCommand::Shell(ShellCommand::ManualRefresh);
        assert_eq!(raised(&mut dom, refresh()).await, None, "a plain failure");

        let stale = BackendError::new(ErrorCode::BuildMismatch, "版本不一致");
        backend.fail("localize_entry_assets", stale.clone());
        let localize = UiCommand::Reader(ReaderCommand::LocalizeEntryAssets { entry_id: 7 });
        assert_eq!(raised(&mut dom, localize).await, Some(AppRecovery::Reload), "unshown");

        backend.fail("load_settings", BackendError::new(ErrorCode::Unauthenticated, "未登录"));
        let shell = UiCommand::Shell(ShellCommand::LoadAuthenticatedShell);
        assert_eq!(raised(&mut dom, shell).await, Some(AppRecovery::SignIn), "outranks reload");

        backend.fail("refresh_all", stale);
        assert_eq!(raised(&mut dom, refresh()).await, Some(AppRecovery::SignIn), "not lowered");
    }
}
