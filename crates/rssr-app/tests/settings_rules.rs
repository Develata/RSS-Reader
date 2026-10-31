//! Compile the production Settings rules without Dioxus, host adapters or substitute state.
//! `scripts/check_settings_rule_boundary.py` also builds this harness with rustc using only
//! the existing API/application/domain dependency graph.
#![allow(dead_code)]

#[path = "../src/ui/failure.rs"]
pub(crate) mod failure;
mod ui {
    pub(crate) use crate::failure;
}

#[path = "../src/pages/settings_page/draft_edits.rs"]
mod draft_edits;
#[path = "../src/pages/settings_page/draft_projection.rs"]
mod draft_projection;
#[path = "../src/pages/settings_page/intent.rs"]
mod intent;
#[path = "../src/pages/settings_page/request_lifecycle.rs"]
mod request_lifecycle;
#[path = "../src/pages/settings_page/save_policy.rs"]
mod save_policy;

use draft_edits::Edits;
use draft_projection::{ProjectionSource, project_stored};
use intent::SettingsPageIntent;
use request_lifecycle::{LoadProjection, SettingsRequestLifecycle};
use rssr_api::{BackendError, ErrorCode};
use rssr_application::SettingsPatch;
use rssr_domain::{DEFAULT_ENTRIES_PAGE_SIZE, ThemeMode, UserSettings};
use save_policy::{SaveCompletion, normalize_submission, saved_status_message, validate_patch};

#[test]
fn save_receipt_keeps_later_fields_and_retires_the_held_read() {
    let stored = UserSettings::default();
    let mut edits = Edits::restored(SettingsPatch {
        entries_page_size: Some(0),
        custom_css: Some("/* submitted */".into()),
        ..Default::default()
    });
    let mut requests = SettingsRequestLifecycle::default();
    let read = requests.begin_load(3);
    assert!(validate_patch(edits.pending()).is_ok());
    assert!(requests.begin_write());
    let mut message = "设置已保存。".to_owned();
    let submitted = normalize_submission(edits.submit(), &mut message);
    assert_eq!(submitted.entries_page_size, Some(DEFAULT_ENTRIES_PAGE_SIZE));
    assert_eq!(edits.over(&stored).entries_page_size, 0, "normalization does not change input");

    edits.record(SettingsPatch {
        refresh_interval_minutes: Some(75),
        custom_css: Some("/* typed later */".into()),
        ..Default::default()
    });
    assert!(requests.receive_load(read, stored.clone()).is_none());
    assert!(!requests.begin_write(), "save and pull cannot overlap");
    // A store receipt can include a field changed elsewhere that this patch did not send.
    let saved = submitted.applied(&UserSettings { theme: ThemeMode::Dark, ..stored });
    let completion = SaveCompletion::collect([
        SettingsPageIntent::SettingsLoaded(saved.clone()),
        SettingsPageIntent::SetStatus { message, tone: "info".into() },
    ]);
    let SaveCompletion::Saved { settings, message } = completion else { panic!("receipt") };
    requests.settings_stored();
    edits.saved();
    let projection = project_stored(
        &edits,
        &settings,
        "/* typed later */",
        [4; 4],
        ProjectionSource::Saved { current_preset: "custom", submitted_preset: "none" },
    );
    assert_eq!(projection.draft.theme, ThemeMode::Dark);
    assert_eq!(projection.draft.entries_page_size, DEFAULT_ENTRIES_PAGE_SIZE);
    assert_eq!(projection.draft.refresh_interval_minutes, 75);
    assert_eq!(projection.draft.custom_css, "/* typed later */");
    assert_eq!(projection.numeric_revisions, [4, 5, 5, 5]);
    assert!(!projection.invalidate_theme_import);
    assert!(!projection.refresh_preset && !projection.clear_css_preview);
    assert!(saved_status_message(message, edits.has_unsaved_changes()).contains("新修改仍保留"));
    assert!(requests.end_write().is_none());
    assert!(!requests.accepts(read), "stale failures/statuses are fenced as well");
    assert_eq!(edits.unsaved(), Some(edits.pending().clone()));
    assert!(requests.begin_write());
    assert_eq!(
        edits.submit().applied(&saved),
        projection.draft,
        "explicit retry sends later edits"
    );
}

#[test]
fn failed_save_restores_edits_over_the_current_source_after_releasing_a_held_read() {
    for code in [ErrorCode::Network, ErrorCode::OutcomeUnknown] {
        for app_changed in [false, true] {
            let mut edits = Edits::restored(SettingsPatch {
                entries_page_size: Some(80),
                custom_css: Some("/* submitted */".into()),
                ..Default::default()
            });
            let mut requests = SettingsRequestLifecycle::default();
            let read = requests.begin_load(7);
            assert!(requests.begin_write());
            edits.submit();
            edits.record(SettingsPatch { entries_page_size: Some(90), ..Default::default() });
            let loaded = UserSettings { refresh_interval_minutes: 60, ..Default::default() };
            assert!(requests.receive_load(read, loaded.clone()).is_none());
            let completion = SaveCompletion::collect([SettingsPageIntent::Failed(
                failure::UiFailure::new("保存设置", BackendError::new(code, "请重新读取确认")),
            )]);
            let SaveCompletion::Failed { message, tone } = completion else { panic!("no receipt") };
            assert_eq!(tone, if code == ErrorCode::OutcomeUnknown { "unknown" } else { "error" });
            assert_eq!(message.contains("保存设置失败"), code != ErrorCode::OutcomeUnknown);
            assert!(message.ends_with("草稿已保留，可修改后重试。"));
            edits.failed();
            let held = requests.end_write().expect("latest read survives a failed save");
            let resolved = held.resolve(if app_changed { 8 } else { 7 });
            let source = if app_changed {
                assert_eq!(
                    resolved,
                    LoadProjection::ShowCurrent,
                    "do not store the old read again"
                );
                UserSettings { refresh_interval_minutes: 120, ..Default::default() }
            } else {
                assert_eq!(resolved, LoadProjection::Apply(loaded.clone()));
                loaded
            };
            let projection = project_stored(
                &edits,
                &source,
                "/* submitted */",
                [2; 4],
                ProjectionSource::Stored,
            );
            assert_eq!(projection.draft.entries_page_size, 90);
            assert_eq!(projection.draft.custom_css, "/* submitted */");
            assert_eq!(projection.draft.refresh_interval_minutes, source.refresh_interval_minutes);
            assert_eq!(projection.numeric_revisions, [3, 3, 2, 3]);
            assert!(!projection.invalidate_theme_import);
            assert!(!projection.refresh_preset && !projection.clear_css_preview);
            assert!(requests.end_write().is_none(), "held read is consumed once");
            assert!(requests.begin_write());
            assert_eq!(edits.submit().applied(&source), projection.draft);
        }
    }
}
