#[cfg(not(target_arch = "wasm32"))]
use dioxus::prelude::spawn;
#[cfg(not(target_arch = "wasm32"))]
use std::future::Future;

use crate::pages::settings_page::facade::SettingsPageFacade;

#[cfg(not(target_arch = "wasm32"))]
use super::theme_file_io::{pick_css_file_contents, save_css_file};

#[cfg(target_arch = "wasm32")]
use super::theme_file_io::save_css_file_in_browser;

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn import_css_file(facade: &SettingsPageFacade) {
    import_css_file_with(facade, pick_css_file_contents());
}

#[cfg(not(target_arch = "wasm32"))]
fn import_css_file_with(
    facade: &SettingsPageFacade,
    read: impl Future<Output = anyhow::Result<Option<String>>> + 'static,
) {
    use super::theme_apply::apply_custom_css_from_raw;

    let facade = facade.clone();
    spawn(async move {
        match read.await {
            Ok(Some(raw)) => {
                apply_custom_css_from_raw(&facade, raw, "已从文件载入并应用自定义 CSS。")
            }
            Ok(None) => facade.set_status("已取消载入 CSS 文件。", "info"),
            Err(err) => facade.set_status(format!("载入 CSS 文件失败：{err}"), "error"),
        }
    });
}

pub(super) fn export_css_file(raw: String, facade: &SettingsPageFacade) {
    if raw.trim().is_empty() {
        facade.set_status("当前没有可导出的自定义 CSS。", "info");
        return;
    }

    #[cfg(target_arch = "wasm32")]
    {
        match save_css_file_in_browser(&raw) {
            Ok(()) => facade.set_status("已导出当前自定义 CSS。", "info"),
            Err(err) => facade.set_status(format!("导出 CSS 文件失败：{err}"), "error"),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        export_css_file_with(raw, facade, |raw| async move { save_css_file(&raw).await });
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn export_css_file_with<F: Future<Output = anyhow::Result<bool>> + 'static>(
    raw: String,
    facade: &SettingsPageFacade,
    save: impl FnOnce(String) -> F + 'static,
) {
    let facade = facade.clone();
    spawn(async move {
        match save(raw).await {
            Ok(true) => facade.set_status("已导出当前自定义 CSS。", "info"),
            Ok(false) => facade.set_status("已取消导出 CSS 文件。", "info"),
            Err(err) => facade.set_status(format!("导出 CSS 文件失败：{err}"), "error"),
        }
    });
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use crate::{
        pages::settings_page::{
            save::{SettingsPageSaveSession, SettingsPageSaveState},
            session::SettingsPageSession,
            sync::{SettingsPageSyncSession, SettingsPageSyncState},
        },
        theme::ThemeController,
    };
    use dioxus::prelude::*;
    use rssr_domain::UserSettings;
    use tokio::sync::oneshot;

    fn harness() -> Element {
        let theme = ThemeController {
            settings: use_signal(|| UserSettings {
                custom_css: "/* persisted */".into(),
                ..UserSettings::default()
            }),
        };
        let page = SettingsPageSession::new(theme);
        let save_state = use_signal(SettingsPageSaveState::new);
        let save = SettingsPageSaveSession::new(save_state, page);
        let sync_state = use_signal(SettingsPageSyncState::new);
        let sync = SettingsPageSyncSession::new(sync_state, page);
        use_context_provider(|| {
            SettingsPageFacade::new(page, save, save.snapshot(), sync, sync.snapshot())
        });
        use_context_provider(|| theme);
        rsx! {}
    }

    fn setup() -> (VirtualDom, SettingsPageFacade, ThemeController) {
        let mut dom = VirtualDom::new(harness);
        dom.rebuild_in_place();
        let (facade, theme) = dom.in_scope(ScopeId::APP, || (consume_context(), consume_context()));
        (dom, facade, theme)
    }

    #[test]
    fn cancelled_or_failed_import_preserves_later_draft_and_applied_theme() {
        for (result, message, tone) in [
            (Ok(None), "已取消载入 CSS 文件。", "info"),
            (Err(anyhow::anyhow!("file vanished")), "载入 CSS 文件失败：file vanished", "error"),
            (Ok(Some("body {".into())), "自定义 CSS 格式无效：", "error"),
        ] {
            let (mut dom, facade, theme) = setup();
            let (sender, receiver) = oneshot::channel();
            dom.in_scope(ScopeId::APP, || {
                import_css_file_with(&facade, async move { receiver.await.unwrap() });
                facade.set_custom_css("/* edited while choosing */".into());
            });
            dom.render_immediate_to_vec();
            sender.send(result).unwrap();
            dom.render_immediate_to_vec();
            dom.in_scope(ScopeId::APP, || {
                assert_eq!(facade.custom_css(), "/* edited while choosing */");
                assert_eq!(theme.settings.peek().custom_css, "/* persisted */");
                assert!(facade.status_message().starts_with(message));
                assert_eq!(facade.status_tone(), tone);
                assert!(!facade.is_save_pending());
            });
        }
    }

    #[test]
    fn export_uses_clicked_draft_and_reports_each_outcome_without_saving_settings() {
        for (result, message, tone) in [
            (Ok(true), "已导出当前自定义 CSS。", "info"),
            (Ok(false), "已取消导出 CSS 文件。", "info"),
            (Err(anyhow::anyhow!("write denied")), "导出 CSS 文件失败：write denied", "error"),
        ] {
            let (mut dom, facade, theme) = setup();
            let (sender, receiver) = oneshot::channel();
            dom.in_scope(ScopeId::APP, || {
                facade.set_custom_css("/* clicked draft */".into());
                export_css_file_with(facade.custom_css(), &facade, |raw| async move {
                    assert_eq!(raw, "/* clicked draft */");
                    receiver.await.unwrap()
                });
                facade.set_custom_css("/* later draft */".into());
            });
            dom.render_immediate_to_vec();
            sender.send(result).unwrap();
            dom.render_immediate_to_vec();
            dom.in_scope(ScopeId::APP, || {
                assert_eq!(facade.custom_css(), "/* later draft */");
                assert_eq!(theme.settings.peek().custom_css, "/* persisted */");
                assert_eq!(facade.status_message(), message);
                assert_eq!(facade.status_tone(), tone);
                assert!(!facade.is_save_pending());
            });
        }
    }

    #[test]
    fn empty_export_reports_no_content_without_opening_a_picker() {
        let (dom, facade, _) = setup();
        dom.in_scope(ScopeId::APP, || {
            export_css_file(" \r\n ".into(), &facade);
            assert_eq!(facade.status_message(), "当前没有可导出的自定义 CSS。");
        });
    }

    #[test]
    fn page_disposal_cancels_import_and_export_waiters() {
        let (mut dom, facade, _) = setup();
        let (import_sender, import_receiver) = oneshot::channel();
        let (export_sender, export_receiver) = oneshot::channel();
        dom.in_scope(ScopeId::APP, || {
            import_css_file_with(&facade, async move { import_receiver.await.unwrap() });
            export_css_file_with(facade.custom_css(), &facade, |_| async move {
                export_receiver.await.unwrap()
            });
        });
        dom.render_immediate_to_vec();
        assert!(!import_sender.is_closed());
        assert!(!export_sender.is_closed());
        drop(dom);
        assert!(import_sender.is_closed());
        assert!(export_sender.is_closed());
    }
}
