use rssr_domain::UserSettings;

use crate::pages::entries_page::intent::EntriesPageIntent;
use crate::pages::feeds_page::intent::FeedsPageIntent;
use crate::pages::reader_page::intent::ReaderPageIntent;
use crate::pages::settings_page::intent::SettingsPageIntent;
use crate::router::AppRoute;
use crate::ui::failure::UiFailure;

#[derive(Debug, Clone)]
pub(crate) struct AuthenticatedShellSnapshot {
    pub(crate) settings: UserSettings,
}

#[derive(Debug, Clone)]
pub(crate) struct StartupRouteSnapshot {
    pub(crate) route: AppRoute,
}

#[derive(Debug, Clone)]
pub(crate) enum UiIntent {
    AuthenticatedShellLoaded(AuthenticatedShellSnapshot),
    StartupRouteResolved(StartupRouteSnapshot),
    EntriesPage(EntriesPageIntent),
    FeedsPage(FeedsPageIntent),
    ReaderPage(ReaderPageIntent),
    SettingsPage(SettingsPageIntent),
    SetStatus {
        message: String,
        tone: String,
    },
    /// A failed backend call (C4b item 1): each page takes it as its own `Failed`, the shell as
    /// its status.
    Failed(UiFailure),
    /// A failure no page shows (the reader's image localization): only what it asks of the whole
    /// app applies (sign in again, reload).
    Unshown(UiFailure),
}

impl UiIntent {
    pub(crate) fn into_status(self) -> Option<(String, String)> {
        match self {
            UiIntent::SetStatus { message, tone } => Some((message, tone)),
            UiIntent::Failed(failure) => Some((failure.message(), failure.tone().to_owned())),
            _ => None,
        }
    }

    /// The backend failure this intent carries, wherever it is.
    pub(crate) fn failure(&self) -> Option<&UiFailure> {
        match self {
            UiIntent::Failed(failure) | UiIntent::Unshown(failure) => Some(failure),
            UiIntent::EntriesPage(intent) => intent.failure(),
            UiIntent::FeedsPage(intent) => intent.failure(),
            UiIntent::ReaderPage(intent) => intent.failure(),
            UiIntent::SettingsPage(intent) => intent.failure(),
            UiIntent::AuthenticatedShellLoaded(_)
            | UiIntent::StartupRouteResolved(_)
            | UiIntent::SetStatus { .. } => None,
        }
    }

    pub(crate) fn into_startup_route_resolved(self) -> Option<StartupRouteSnapshot> {
        match self {
            UiIntent::StartupRouteResolved(snapshot) => Some(snapshot),
            _ => None,
        }
    }

    pub(crate) fn into_entries_page_intent(self) -> Option<EntriesPageIntent> {
        match self {
            UiIntent::EntriesPage(intent) => Some(intent),
            UiIntent::SetStatus { message, tone } => {
                Some(EntriesPageIntent::SetStatus { message, tone })
            }
            UiIntent::Failed(failure) => Some(EntriesPageIntent::Failed(failure)),
            UiIntent::Unshown(_) => None,
            UiIntent::AuthenticatedShellLoaded(_)
            | UiIntent::StartupRouteResolved(_)
            | UiIntent::FeedsPage(_)
            | UiIntent::ReaderPage(_)
            | UiIntent::SettingsPage(_) => None,
        }
    }

    pub(crate) fn into_reader_page_intent(self) -> Option<ReaderPageIntent> {
        match self {
            UiIntent::ReaderPage(intent) => Some(intent),
            UiIntent::SetStatus { message, tone } => {
                Some(ReaderPageIntent::SetStatus { message, tone })
            }
            UiIntent::Failed(failure) => Some(ReaderPageIntent::Failed(failure)),
            UiIntent::Unshown(_) => None,
            UiIntent::AuthenticatedShellLoaded(_)
            | UiIntent::StartupRouteResolved(_)
            | UiIntent::EntriesPage(_)
            | UiIntent::FeedsPage(_)
            | UiIntent::SettingsPage(_) => None,
        }
    }

    pub(crate) fn into_feeds_page_intent(self) -> Option<FeedsPageIntent> {
        match self {
            UiIntent::FeedsPage(intent) => Some(intent),
            UiIntent::SetStatus { message, tone } => {
                Some(FeedsPageIntent::SetStatus { message, tone })
            }
            UiIntent::Failed(failure) => Some(FeedsPageIntent::Failed(failure)),
            UiIntent::Unshown(_) => None,
            UiIntent::AuthenticatedShellLoaded(_)
            | UiIntent::StartupRouteResolved(_)
            | UiIntent::EntriesPage(_)
            | UiIntent::ReaderPage(_)
            | UiIntent::SettingsPage(_) => None,
        }
    }

    pub(crate) fn into_settings_page_intent(self) -> Option<SettingsPageIntent> {
        match self {
            UiIntent::SettingsPage(intent) => Some(intent),
            UiIntent::SetStatus { message, tone } => {
                Some(SettingsPageIntent::SetStatus { message, tone })
            }
            UiIntent::Failed(failure) => Some(SettingsPageIntent::Failed(failure)),
            UiIntent::Unshown(_) => None,
            UiIntent::AuthenticatedShellLoaded(_)
            | UiIntent::StartupRouteResolved(_)
            | UiIntent::EntriesPage(_)
            | UiIntent::FeedsPage(_)
            | UiIntent::ReaderPage(_) => None,
        }
    }
}
