mod appearance;
mod browser;
mod facade;
pub(crate) mod intent;
mod preferences;
mod save;
mod session;
mod sync;
mod themes;

use dioxus::prelude::*;

use self::{
    appearance::AppearanceSettingsCard,
    facade::SettingsPageFacade,
    save::{SettingsPageSaveSession, SettingsPageSaveState},
    session::SettingsPageSession,
    sync::{SettingsPageSyncSession, SettingsPageSyncState, WebDavSettingsCard},
};
use crate::{
    app::AppNav, components::status_banner::StatusBanner,
    hooks::use_mobile_back_navigation::use_mobile_back_navigation, theme::ThemeController,
    ui::use_reactive_task,
};

#[component]
pub fn SettingsPage() -> Element {
    use_mobile_back_navigation(Some(crate::router::AppRoute::EntriesPage {}));

    let theme = use_context::<ThemeController>();
    let session = SettingsPageSession::new(theme);
    let save_state = use_signal(SettingsPageSaveState::new);
    let save_session = SettingsPageSaveSession::new(save_state, session);
    let sync_state = use_signal(SettingsPageSyncState::new);
    let sync_session = SettingsPageSyncSession::new(sync_state, session);
    let facade = SettingsPageFacade::new(
        session,
        save_session,
        save_session.snapshot(),
        sync_session,
        sync_session.snapshot(),
    );
    let repository_facade = facade.clone();

    use_reactive_task((), move |_| {
        session.load();
    });

    rsx! {
        section { "data-page": "settings",
            AppNav {}
            div { "data-slot": "page-section-header", "data-layout": "page-header", "data-section": "settings",
                h2 { "data-slot": "page-title", "设置" }
                div { "data-slot": "page-header-actions",
                    button {
                        class: "icon-link-button",
                        "data-action": "open-github-repo",
                        r#type: "button",
                        aria_label: "打开项目 GitHub 仓库",
                        title: "打开项目 GitHub 仓库",
                        onclick: move |_| repository_facade.open_repository(),
                        svg {
                            xmlns: "http://www.w3.org/2000/svg",
                            view_box: "0 0 24 24",
                            width: "18",
                            height: "18",
                            "aria-hidden": "true",
                            fill: "currentColor",
                            path {
                                d: "M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.009-.866-.014-1.7-2.782.605-3.369-1.344-3.369-1.344-.455-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.071 1.531 1.032 1.531 1.032.892 1.53 2.341 1.088 2.91.832.091-.647.349-1.088.635-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.027A9.564 9.564 0 0 1 12 6.844c.85.004 1.705.115 2.504.337 1.909-1.297 2.748-1.027 2.748-1.027.546 1.378.203 2.397.1 2.65.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.31.678.921.678 1.857 0 1.34-.012 2.422-.012 2.75 0 .268.18.58.688.481A10.02 10.02 0 0 0 22 12.017C22 6.484 17.523 2 12 2z"
                            }
                        }
                    }
                }
            }
            if facade.has_status_message() {
                StatusBanner { message: facade.status_message(), tone: facade.status_tone() }
            }
            div { "data-layout": "settings-grid",
                AppearanceSettingsCard { facade: facade.clone() }
                WebDavSettingsCard { facade }
            }
        }
    }
}
