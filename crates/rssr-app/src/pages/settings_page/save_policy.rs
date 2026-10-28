//! Page-local save rules. Signals, the synchronous pending gate and task ownership
//! stay in the sessions; these functions neither persist nor retain another draft.
use rssr_domain::{DEFAULT_ENTRIES_PAGE_SIZE, UserSettings};

use super::{
    intent::SettingsPageIntent,
    themes::{detect_preset_key, validate_custom_css},
};

pub(super) struct PreparedSave {
    pub settings: UserSettings,
    pub success_message: String,
}

pub(super) fn prepare_save(
    mut settings: UserSettings,
    mut success_message: String,
) -> Result<PreparedSave, String> {
    validate_custom_css(&settings.custom_css)
        .map_err(|err| format!("自定义 CSS 格式无效：{err}"))?;
    if settings.entries_page_size == 0 {
        settings.entries_page_size = DEFAULT_ENTRIES_PAGE_SIZE;
        success_message.push_str(&format!(
            " 文章页每页数量输入为 0，已回退为默认值 {DEFAULT_ENTRIES_PAGE_SIZE}。"
        ));
    }
    Ok(PreparedSave { settings, success_message })
}

pub(super) enum SaveCompletion {
    Saved { settings: UserSettings, message: String },
    Failed { message: String },
}

impl SaveCompletion {
    /// The last settings and the last status win independently. A settings result,
    /// not the incoming status tone or ordering, determines save success.
    pub(super) fn collect(intents: impl IntoIterator<Item = SettingsPageIntent>) -> Self {
        let mut saved_settings = None;
        let mut message = String::new();
        for intent in intents {
            match intent {
                SettingsPageIntent::SettingsLoaded(settings) => saved_settings = Some(settings),
                SettingsPageIntent::SetStatus { message: next, .. } => message = next,
            }
        }
        match saved_settings {
            Some(settings) => Self::Saved { settings, message },
            None => {
                message.push_str(" 草稿已保留，可修改后重试。");
                Self::Failed { message }
            }
        }
    }
}

pub(super) fn saved_status_message(mut message: String, has_unsaved_edits: bool) -> String {
    if has_unsaved_edits {
        message.push_str(" 保存期间的新修改仍保留在草稿中，尚未保存。");
    }
    message
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct SavedDraftUpdate {
    pub replace_draft: bool,
    pub preset_choice: Option<&'static str>,
    pub has_unsaved_edits: bool,
}

pub(super) fn saved_draft_update(
    saved: &UserSettings,
    submitted: &UserSettings,
    submitted_preset: &str,
    current: &UserSettings,
    current_preset: &str,
) -> SavedDraftUpdate {
    // Whole-value equality is intentional: later edits are never merged field by
    // field. Editing away and back to the submitted value permits replacement.
    let replace_draft = current == submitted;
    let effective_draft = if replace_draft { saved } else { current };
    let preset_choice = (current_preset == submitted_preset)
        .then(|| detect_preset_key(&effective_draft.custom_css));
    SavedDraftUpdate { replace_draft, preset_choice, has_unsaved_edits: effective_draft != saved }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pages::settings_page::themes::preset_css;
    use rssr_domain::ThemeMode;

    #[test]
    fn preparation_preserves_values_and_only_normalizes_zero_page_size() {
        for page_size in [0, 1, 200, 201] {
            let submitted = UserSettings {
                entries_page_size: page_size,
                refresh_interval_minutes: 0,
                custom_css: "/* valid CSS */".into(),
                ..UserSettings::default()
            };
            let prepared = prepare_save(submitted.clone(), "saved".into()).unwrap();
            let mut expected = submitted;
            expected.entries_page_size = if page_size == 0 { 100 } else { page_size };
            assert_eq!(prepared.settings, expected, "other validation belongs to application");
            assert_eq!(
                prepared.success_message,
                if page_size == 0 {
                    "saved 文章页每页数量输入为 0，已回退为默认值 100。"
                } else {
                    "saved"
                }
            );
        }
    }

    #[test]
    fn malformed_css_is_rejected_with_the_existing_domain_reason() {
        let settings = UserSettings { custom_css: "}".into(), ..UserSettings::default() };
        assert_eq!(
            prepare_save(settings, "saved".into()).err().unwrap(),
            "自定义 CSS 格式无效：存在未匹配的右括号或右花括号"
        );
    }

    #[test]
    fn untouched_draft_receives_saved_normalization_and_preset_from_saved_css() {
        let submitted = UserSettings { entries_page_size: 0, ..UserSettings::default() };
        let saved =
            UserSettings { custom_css: preset_css("newsprint").into(), ..UserSettings::default() };
        assert_eq!(
            saved_draft_update(&saved, &submitted, "none", &submitted, "none"),
            SavedDraftUpdate {
                replace_draft: true,
                preset_choice: Some("newsprint"),
                has_unsaved_edits: false,
            }
        );
    }

    #[test]
    fn later_edit_keeps_the_whole_draft_and_detects_preset_from_retained_css() {
        let submitted = UserSettings { entries_page_size: 0, ..UserSettings::default() };
        let saved = UserSettings::default();
        let current =
            UserSettings { custom_css: preset_css("newsprint").into(), ..submitted.clone() };
        assert_eq!(
            saved_draft_update(&saved, &submitted, "none", &current, "none"),
            SavedDraftUpdate {
                replace_draft: false,
                preset_choice: Some("newsprint"),
                has_unsaved_edits: true,
            }
        );
        assert_eq!(current.entries_page_size, 0);
    }

    #[test]
    fn new_preset_selection_is_independent_of_draft_replacement_and_unsaved_edits() {
        let submitted = UserSettings { entries_page_size: 0, ..UserSettings::default() };
        let saved = UserSettings::default();
        for edited in [false, true] {
            let current = UserSettings {
                theme: if edited { ThemeMode::Dark } else { submitted.theme },
                ..submitted.clone()
            };
            assert_eq!(
                saved_draft_update(&saved, &submitted, "none", &current, "atlas-sidebar"),
                SavedDraftUpdate {
                    replace_draft: !edited,
                    preset_choice: None,
                    has_unsaved_edits: edited,
                }
            );
        }
    }

    #[test]
    fn draft_comparison_uses_values_without_a_new_edit_generation() {
        let submitted = UserSettings { entries_page_size: 0, ..UserSettings::default() };
        let saved = UserSettings::default();
        let mut current = submitted.clone();
        current.theme = ThemeMode::Dark;
        assert!(!saved_draft_update(&saved, &submitted, "none", &current, "none").replace_draft);
        current.theme = submitted.theme;
        assert!(saved_draft_update(&saved, &submitted, "none", &current, "none").replace_draft);
        let already_saved = saved_draft_update(&saved, &submitted, "none", &saved, "none");
        assert!(!already_saved.replace_draft);
        assert!(!already_saved.has_unsaved_edits, "current value already equals persisted result");
    }

    #[test]
    fn completion_uses_last_settings_and_last_message_independently_of_tone_or_order() {
        let first = UserSettings::default();
        let last = UserSettings { theme: ThemeMode::Dark, ..first.clone() };
        for status_first in [false, true] {
            let mut intents = vec![
                SettingsPageIntent::SettingsLoaded(first.clone()),
                SettingsPageIntent::SetStatus { message: "old".into(), tone: "info".into() },
            ];
            let latest = SettingsPageIntent::SettingsLoaded(last.clone());
            let status =
                SettingsPageIntent::SetStatus { message: "last".into(), tone: "error".into() };
            intents.extend(if status_first { [status, latest] } else { [latest, status] });
            let SaveCompletion::Saved { settings, message } = SaveCompletion::collect(intents)
            else {
                panic!("settings result determines success")
            };
            assert_eq!(settings, last);
            assert_eq!(message, "last");
        }
    }

    #[test]
    fn completion_without_settings_keeps_the_draft_even_for_empty_or_info_results() {
        for (intents, expected) in [
            (vec![], " 草稿已保留，可修改后重试。"),
            (
                vec![SettingsPageIntent::SetStatus {
                    message: "failed".into(),
                    tone: "info".into(),
                }],
                "failed 草稿已保留，可修改后重试。",
            ),
        ] {
            let SaveCompletion::Failed { message } = SaveCompletion::collect(intents) else {
                panic!("no persisted settings")
            };
            assert_eq!(message, expected);
        }
    }

    #[test]
    fn saved_feedback_only_mentions_edits_that_differ_from_persisted_settings() {
        let SaveCompletion::Saved { message, .. } =
            SaveCompletion::collect([SettingsPageIntent::SettingsLoaded(UserSettings::default())])
        else {
            panic!("settings without a message still succeed")
        };
        assert_eq!(saved_status_message(message, false), "");
        assert_eq!(saved_status_message("saved".into(), false), "saved");
        assert_eq!(
            saved_status_message("saved".into(), true),
            "saved 保存期间的新修改仍保留在草稿中，尚未保存。"
        );
    }
}
