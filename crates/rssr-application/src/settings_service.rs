use std::sync::Arc;

use anyhow::Result;
use rssr_domain::{
    DomainError, ListDensity, SettingsRepository, StartupView, ThemeMode, UserSettings,
    validate_user_settings, validation::validate_custom_css,
};
use serde::{Deserialize, Serialize};

use crate::persistence_mutation::SettingsUpdatePort;

/// A change to some settings fields; an absent field keeps its stored value.
///
/// `custom_css: Some("")` clears the custom CSS. Unknown fields are refused when deserialized.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SettingsPatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<ThemeMode>,
    /// Legacy 0.2.x API/CLI compatibility; ignored by the UI and removed in 0.3.x.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_density: Option<ListDensity>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub startup_view: Option<StartupView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_interval_minutes: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archive_after_months: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entries_page_size: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reader_font_scale: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_css: Option<String>,
}

impl SettingsPatch {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// The fields of `edited` that differ from `base`.
    pub fn between(base: &UserSettings, edited: &UserSettings) -> Self {
        fn changed<T: PartialEq + Clone>(base: &T, edited: &T) -> Option<T> {
            (base != edited).then(|| edited.clone())
        }
        Self {
            theme: changed(&base.theme, &edited.theme),
            list_density: changed(&base.list_density, &edited.list_density),
            startup_view: changed(&base.startup_view, &edited.startup_view),
            refresh_interval_minutes: changed(
                &base.refresh_interval_minutes,
                &edited.refresh_interval_minutes,
            ),
            archive_after_months: changed(&base.archive_after_months, &edited.archive_after_months),
            entries_page_size: changed(&base.entries_page_size, &edited.entries_page_size),
            reader_font_scale: changed(&base.reader_font_scale, &edited.reader_font_scale),
            custom_css: changed(&base.custom_css, &edited.custom_css),
        }
    }

    /// This patch with `later`'s fields put in: a field `later` sets wins.
    pub fn followed_by(self, later: SettingsPatch) -> Self {
        Self {
            theme: later.theme.or(self.theme),
            list_density: later.list_density.or(self.list_density),
            startup_view: later.startup_view.or(self.startup_view),
            refresh_interval_minutes: later
                .refresh_interval_minutes
                .or(self.refresh_interval_minutes),
            archive_after_months: later.archive_after_months.or(self.archive_after_months),
            entries_page_size: later.entries_page_size.or(self.entries_page_size),
            reader_font_scale: later.reader_font_scale.or(self.reader_font_scale),
            custom_css: later.custom_css.or(self.custom_css),
        }
    }

    /// `settings` with this patch's fields put in; nothing is checked.
    pub fn applied(&self, settings: &UserSettings) -> UserSettings {
        let mut next = settings.clone();
        if let Some(theme) = self.theme {
            next.theme = theme;
        }
        if let Some(list_density) = self.list_density {
            next.list_density = list_density;
        }
        if let Some(startup_view) = self.startup_view {
            next.startup_view = startup_view;
        }
        if let Some(minutes) = self.refresh_interval_minutes {
            next.refresh_interval_minutes = minutes;
        }
        if let Some(months) = self.archive_after_months {
            next.archive_after_months = months;
        }
        if let Some(page_size) = self.entries_page_size {
            next.entries_page_size = page_size;
        }
        if let Some(scale) = self.reader_font_scale {
            next.reader_font_scale = scale;
        }
        if let Some(custom_css) = &self.custom_css {
            next.custom_css.clone_from(custom_css);
        }
        next
    }

    /// The one merge rule both stores apply inside their write: put the fields in, check the
    /// ranges of the result, and check the custom CSS only when this patch sets it (stored CSS
    /// from before the check never blocks an unrelated change).
    pub fn merge(&self, current: &UserSettings) -> rssr_domain::Result<UserSettings> {
        let next = self.applied(current);
        validate_user_settings(&next)?;
        if let Some(custom_css) = &self.custom_css {
            validate_custom_css(custom_css).map_err(|reason| {
                DomainError::InvalidInput(format!("自定义 CSS 格式无效：{reason}"))
            })?;
        }
        Ok(next)
    }
}

#[derive(Clone)]
pub struct SettingsService {
    repository: Arc<dyn SettingsRepository>,
    updates: Arc<dyn SettingsUpdatePort>,
}

impl SettingsService {
    pub fn new(
        repository: Arc<dyn SettingsRepository>,
        updates: Arc<dyn SettingsUpdatePort>,
    ) -> Self {
        Self { repository, updates }
    }

    pub async fn load(&self) -> Result<UserSettings> {
        Ok(self.repository.load().await?)
    }

    /// Merges, checks and saves `patch` in one write of the store and returns the settings now
    /// stored. An empty patch writes nothing and returns the current settings.
    pub async fn update(&self, patch: SettingsPatch) -> Result<UserSettings> {
        if patch.is_empty() {
            return self.load().await;
        }
        self.updates.update_settings(patch).await
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::sync::{Arc, Mutex};

    use rssr_domain::{SettingsRepository, ThemeMode, UserSettings};

    use super::{SettingsPatch, SettingsService};
    use crate::persistence_mutation::SettingsUpdatePort;

    /// Settings in memory; `update_settings` follows the stores (merge, save on change).
    #[derive(Default)]
    pub(crate) struct MemorySettings {
        pub(crate) settings: Mutex<UserSettings>,
        pub(crate) writes: Mutex<usize>,
    }

    #[async_trait::async_trait]
    impl SettingsRepository for MemorySettings {
        async fn load(&self) -> rssr_domain::Result<UserSettings> {
            Ok(self.settings.lock().unwrap().clone())
        }

        async fn save(&self, settings: &UserSettings) -> rssr_domain::Result<()> {
            *self.settings.lock().unwrap() = settings.clone();
            Ok(())
        }
    }

    #[async_trait::async_trait]
    impl SettingsUpdatePort for MemorySettings {
        async fn update_settings(&self, patch: SettingsPatch) -> anyhow::Result<UserSettings> {
            let mut settings = self.settings.lock().unwrap();
            let next = patch.merge(&settings)?;
            if next != *settings {
                *settings = next.clone();
                *self.writes.lock().unwrap() += 1;
            }
            Ok(next)
        }
    }

    fn service(settings: UserSettings) -> (SettingsService, Arc<MemorySettings>) {
        let store =
            Arc::new(MemorySettings { settings: Mutex::new(settings), ..Default::default() });
        (SettingsService::new(store.clone(), store.clone()), store)
    }

    #[test]
    fn the_patch_between_two_settings_carries_only_the_differences() {
        let base = UserSettings::default();
        let edited = UserSettings {
            theme: ThemeMode::Dark,
            custom_css: "a { color: red }".into(),
            ..base.clone()
        };
        let patch = SettingsPatch::between(&base, &edited);
        assert_eq!(
            patch,
            SettingsPatch {
                theme: Some(ThemeMode::Dark),
                custom_css: Some("a { color: red }".into()),
                ..SettingsPatch::default()
            }
        );
        assert_eq!(patch.applied(&base), edited);
        assert!(SettingsPatch::between(&base, &base).is_empty());
    }

    /// A later patch's fields win, including one set back to a default value; the others stay.
    #[test]
    fn a_later_patch_wins_field_by_field() {
        let earlier = SettingsPatch {
            theme: Some(ThemeMode::Dark),
            refresh_interval_minutes: Some(90),
            ..SettingsPatch::default()
        };
        let later = SettingsPatch {
            theme: Some(ThemeMode::System),
            custom_css: Some(String::new()),
            ..SettingsPatch::default()
        };
        assert_eq!(
            earlier.followed_by(later),
            SettingsPatch {
                theme: Some(ThemeMode::System),
                refresh_interval_minutes: Some(90),
                custom_css: Some(String::new()),
                ..SettingsPatch::default()
            }
        );
    }

    #[test]
    fn merge_checks_the_ranges_of_the_result() {
        for page_size in [0, 201] {
            let patch = SettingsPatch { entries_page_size: Some(page_size), ..Default::default() };
            let error = patch.merge(&UserSettings::default()).expect_err("out of range");
            assert!(error.to_string().contains("文章页每页数量"));
        }
    }

    /// Stored CSS that never passed the check does not block other fields; CSS in the patch is
    /// checked.
    #[test]
    fn merge_checks_custom_css_only_when_the_patch_sets_it() {
        let stored = UserSettings { custom_css: "a { color: red;".into(), ..Default::default() };
        let theme = SettingsPatch { theme: Some(ThemeMode::Dark), ..Default::default() };
        assert_eq!(theme.merge(&stored).unwrap().theme, ThemeMode::Dark);

        let css =
            SettingsPatch { custom_css: Some("a { color: red;".into()), ..Default::default() };
        assert!(css.merge(&UserSettings::default()).unwrap_err().to_string().contains("CSS"));
        let clear = SettingsPatch { custom_css: Some(String::new()), ..Default::default() };
        assert_eq!(clear.merge(&stored).unwrap().custom_css, "");
    }

    #[test]
    fn a_patch_refuses_unknown_fields() {
        assert!(serde_json::from_str::<SettingsPatch>(r#"{"theme":"dark"}"#).is_ok());
        assert!(serde_json::from_str::<SettingsPatch>(r#"{"colour":"red"}"#).is_err());
    }

    #[tokio::test]
    async fn update_returns_the_stored_settings_and_an_empty_patch_writes_nothing() {
        let (service, store) = service(UserSettings::default());

        let unchanged = service.update(SettingsPatch::default()).await.unwrap();
        assert_eq!(unchanged, UserSettings::default());
        assert_eq!(*store.writes.lock().unwrap(), 0);

        let patch = SettingsPatch { refresh_interval_minutes: Some(45), ..Default::default() };
        let saved = service.update(patch.clone()).await.unwrap();
        assert_eq!(saved.refresh_interval_minutes, 45);
        assert_eq!(*store.writes.lock().unwrap(), 1);
        service.update(patch).await.unwrap();
        assert_eq!(*store.writes.lock().unwrap(), 1, "the same value again writes nothing");
    }
}
