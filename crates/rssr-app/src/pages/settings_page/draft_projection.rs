//! Projection of stored settings through the existing edit overlays. No state owner or effects:
//! the session publishes the draft, retires import/field feedback and updates the preview.
use rssr_domain::UserSettings;

use super::draft_edits::Edits;

pub(super) enum ProjectionSource<'a> {
    /// A read or a pull leaves both submitted and pending edits on top.
    Stored,
    /// The acknowledged edits have already been retired by `Edits::saved`.
    Saved { current_preset: &'a str, submitted_preset: &'a str },
}

pub(super) struct DraftProjection {
    pub(super) draft: UserSettings,
    pub(super) numeric_revisions: [u64; 4],
    pub(super) invalidate_theme_import: bool,
    pub(super) refresh_preset: bool,
    pub(super) clear_css_preview: bool,
}

pub(super) fn project_stored(
    edits: &Edits,
    stored: &UserSettings,
    previous_css: &str,
    numeric_revisions: [u64; 4],
    source: ProjectionSource<'_>,
) -> DraftProjection {
    let draft = edits.over(stored);
    let edited = edits.numeric_edits();
    // Equal stored values still retire old parse feedback; edited fields keep theirs.
    let numeric_revisions = std::array::from_fn(|i| {
        if edited[i] { numeric_revisions[i] } else { numeric_revisions[i].wrapping_add(1) }
    });
    let (refresh_preset, clear_css_preview) = match source {
        ProjectionSource::Stored => (!edits.edit_custom_css(), !edits.edit_custom_css()),
        ProjectionSource::Saved { current_preset, submitted_preset } => {
            (current_preset == submitted_preset, edits.pending().custom_css.is_none())
        }
    };
    DraftProjection {
        invalidate_theme_import: draft.custom_css != previous_css,
        draft,
        numeric_revisions,
        refresh_preset,
        clear_css_preview,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rssr_application::SettingsPatch;

    #[test]
    fn equal_values_and_a_b_a_retire_only_feedback_supplied_by_storage() {
        let mut edits = Edits::default();
        edits.record(SettingsPatch { entries_page_size: Some(80), ..Default::default() });
        let mut revisions = [0; 4];
        for refresh in [30, 90, 30] {
            let stored = UserSettings { refresh_interval_minutes: refresh, ..Default::default() };
            let projection =
                project_stored(&edits, &stored, "", revisions, ProjectionSource::Stored);
            assert_eq!(projection.draft.entries_page_size, 80);
            assert_eq!(projection.draft.refresh_interval_minutes, refresh);
            assert_eq!(projection.numeric_revisions[2], 0, "edited field owns its feedback");
            assert_eq!(projection.numeric_revisions[0], revisions[0] + 1);
            revisions = projection.numeric_revisions;
        }
        assert_eq!(revisions, [3, 3, 0, 3]);
    }

    #[test]
    fn stored_css_only_replaces_preview_and_preset_when_the_field_is_unedited() {
        let stored = UserSettings { custom_css: "/* remote */".into(), ..Default::default() };
        let mut edits = Edits::default();
        let projection = project_stored(&edits, &stored, "", [0; 4], ProjectionSource::Stored);
        assert!(projection.invalidate_theme_import);
        assert!(projection.refresh_preset && projection.clear_css_preview);

        let same =
            project_stored(&edits, &stored, &stored.custom_css, [0; 4], ProjectionSource::Stored);
        assert!(!same.invalidate_theme_import, "equal CSS does not retire a file read");
        edits
            .record(SettingsPatch { custom_css: Some("/* local */".into()), ..Default::default() });
        edits.submit();
        let submitted =
            project_stored(&edits, &stored, "/* local */", [0; 4], ProjectionSource::Stored);
        assert_eq!(submitted.draft.custom_css, "/* local */");
        assert!(!submitted.invalidate_theme_import);
        assert!(!submitted.refresh_preset && !submitted.clear_css_preview);
    }

    #[test]
    fn save_keeps_a_later_preset_choice_independently_of_css_preview_ownership() {
        let stored = UserSettings { custom_css: "/* saved */".into(), ..Default::default() };
        let mut edits = Edits::default();
        let source =
            || ProjectionSource::Saved { current_preset: "custom", submitted_preset: "none" };
        let selected = project_stored(&edits, &stored, "", [0; 4], source());
        assert!(!selected.refresh_preset, "a later choice is not replaced by the receipt");
        assert!(selected.clear_css_preview);

        edits.record(SettingsPatch {
            custom_css: Some("/* typed later */".into()),
            ..Default::default()
        });
        let typed = project_stored(&edits, &stored, "/* typed later */", [0; 4], source());
        assert!(!typed.refresh_preset && !typed.clear_css_preview);
        let same_choice = project_stored(
            &edits,
            &stored,
            "/* typed later */",
            [0; 4],
            ProjectionSource::Saved { current_preset: "none", submitted_preset: "none" },
        );
        assert!(same_choice.refresh_preset, "retain the existing value-equality rule");
        assert!(!same_choice.clear_css_preview, "typing still owns the unsaved CSS");
    }
}
