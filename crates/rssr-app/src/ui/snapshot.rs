use rssr_domain::UserSettings;

use crate::pages::entries_page::intent::EntriesPageIntent;
use crate::pages::feeds_page::intent::FeedsPageIntent;
use crate::pages::reader_page::intent::ReaderPageIntent;
use crate::pages::settings_page::intent::SettingsPageIntent;
use crate::router::AppRoute;

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
    SetStatus { message: String, tone: String },
}

impl UiIntent {
    pub(crate) fn into_status(self) -> Option<(String, String)> {
        match self {
            UiIntent::SetStatus { message, tone } => Some((message, tone)),
            _ => None,
        }
    }

    pub(crate) fn into_authenticated_shell_loaded(self) -> Option<AuthenticatedShellSnapshot> {
        match self {
            UiIntent::AuthenticatedShellLoaded(snapshot) => Some(snapshot),
            _ => None,
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
            UiIntent::AuthenticatedShellLoaded(_)
            | UiIntent::StartupRouteResolved(_)
            | UiIntent::EntriesPage(_)
            | UiIntent::FeedsPage(_)
            | UiIntent::ReaderPage(_) => None,
        }
    }
}
