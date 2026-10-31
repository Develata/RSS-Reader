//! Preparation and receipt folding for per-field SettingsPatch saves. Draft overlays
//! live in draft_edits, read/write ordering in request_lifecycle; the session owns effects.
use super::intent::SettingsPageIntent;
use rssr_application::SettingsPatch;
use rssr_domain::{DEFAULT_ENTRIES_PAGE_SIZE, UserSettings, validation::validate_custom_css};

pub(super) fn validate_patch(patch: &SettingsPatch) -> Result<(), String> {
    if let Some(css) = &patch.custom_css {
        validate_custom_css(css).map_err(|err| format!("自定义 CSS 格式无效：{err}"))?;
    }
    Ok(())
}

pub(super) fn normalize_submission(
    mut patch: SettingsPatch,
    message: &mut String,
) -> SettingsPatch {
    if patch.entries_page_size == Some(0) {
        patch.entries_page_size = Some(DEFAULT_ENTRIES_PAGE_SIZE);
        message.push_str(&format!(
            " 文章页每页数量输入为 0，已回退为默认值 {DEFAULT_ENTRIES_PAGE_SIZE}。"
        ));
    }
    patch
}

pub(super) enum SaveCompletion {
    Saved { settings: UserSettings, message: String },
    Failed { message: String, tone: &'static str },
}

impl SaveCompletion {
    pub(super) fn collect(intents: impl IntoIterator<Item = SettingsPageIntent>) -> Self {
        let mut saved = None;
        let mut message = String::new();
        let mut tone = "error";
        for intent in intents {
            match intent {
                SettingsPageIntent::SettingsLoaded(settings) => saved = Some(settings),
                SettingsPageIntent::SetStatus { message: next, .. } => message = next,
                SettingsPageIntent::Failed(failure) => {
                    message = failure.message();
                    tone = failure.tone();
                }
            }
        }
        match saved {
            Some(settings) => Self::Saved { settings, message },
            None => {
                message.push_str(" 草稿已保留，可修改后重试。");
                Self::Failed { message, tone }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preparation_only_checks_and_normalizes_present_fields() {
        let empty = SettingsPatch::default();
        assert!(validate_patch(&empty).is_ok());
        let mut message = "saved".to_owned();
        assert!(normalize_submission(empty, &mut message).is_empty());
        assert_eq!(message, "saved");
        let patch = SettingsPatch { entries_page_size: Some(0), ..Default::default() };
        assert_eq!(
            normalize_submission(patch, &mut message).entries_page_size,
            Some(DEFAULT_ENTRIES_PAGE_SIZE)
        );
        assert!(message.contains("已回退"));
        assert!(
            validate_patch(&SettingsPatch { custom_css: Some("}".into()), ..Default::default() })
                .is_err()
        );
    }

    #[test]
    fn a_status_alone_is_not_a_save_receipt() {
        let complete = SaveCompletion::collect([SettingsPageIntent::SetStatus {
            message: "info".into(),
            tone: "info".into(),
        }]);
        let SaveCompletion::Failed { message, tone } = complete else {
            panic!("missing stored settings")
        };
        assert_eq!(tone, "error");
        assert!(message.ends_with("草稿已保留，可修改后重试。"));
    }

    #[test]
    fn latest_stored_settings_and_message_win_independently() {
        let settings = UserSettings { entries_page_size: 80, ..Default::default() };
        let complete = SaveCompletion::collect([
            SettingsPageIntent::SettingsLoaded(UserSettings::default()),
            SettingsPageIntent::SetStatus { message: "first".into(), tone: "info".into() },
            SettingsPageIntent::SettingsLoaded(settings.clone()),
            SettingsPageIntent::SetStatus { message: "last".into(), tone: "info".into() },
        ]);
        let SaveCompletion::Saved { settings: saved, message } = complete else { panic!("stored") };
        assert_eq!(saved, settings);
        assert_eq!(message, "last");
    }
}
