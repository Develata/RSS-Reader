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
