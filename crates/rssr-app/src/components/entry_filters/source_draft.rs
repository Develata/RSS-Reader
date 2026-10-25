//! Source-filter editing policy; no UI, storage, or host dependencies.
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Selection {
    Empty,
    Partial,
    All,
}

#[derive(Default)]
pub(super) struct SourceFilterDraft {
    available: BTreeSet<String>,
    selected: BTreeSet<String>,
    search: String,
    query: String,
}

impl SourceFilterDraft {
    pub(super) fn begin(available: BTreeSet<String>, applied: &[String]) -> Self {
        let selected = if applied.is_empty() {
            available.clone()
        } else {
            applied.iter().filter(|url| available.contains(*url)).cloned().collect()
        };
        Self { available, selected, ..Self::default() }
    }

    pub(super) fn reconcile(&mut self, available: BTreeSet<String>) {
        // New sources stay unchecked while editing; removed sources cannot
        // keep the count/apply button alive or leak into an applied filter.
        self.selected.retain(|url| available.contains(url));
        self.available = available;
    }

    pub(super) fn clear(&mut self) {
        *self = Self::default();
    }

    pub(super) fn selected_count(&self) -> usize {
        self.selected.len()
    }

    pub(super) fn selection(&self) -> Selection {
        if self.selected.is_empty() {
            Selection::Empty
        } else if self.selected == self.available {
            Selection::All
        } else {
            Selection::Partial
        }
    }

    pub(super) fn is_selected(&self, url: &str) -> bool {
        self.selected.contains(url)
    }

    pub(super) fn set_selected(&mut self, url: &str, selected: bool) {
        if selected && self.available.contains(url) {
            self.selected.insert(url.to_owned());
        } else {
            self.selected.remove(url);
        }
    }

    pub(super) fn toggle_all(&mut self) {
        if self.selection() == Selection::All {
            self.selected.clear();
        } else {
            self.selected.clone_from(&self.available);
        }
    }

    /// None means no valid choice. Some([]) preserves the existing "all" wire
    /// representation; an empty draft must never silently become "all".
    pub(super) fn apply_value(&self) -> Option<Vec<String>> {
        match self.selection() {
            Selection::Empty => None,
            Selection::All => Some(Vec::new()),
            Selection::Partial => Some(self.selected.iter().cloned().collect()),
        }
    }

    pub(super) fn search(&self) -> &str {
        &self.search
    }

    pub(super) fn set_search(&mut self, search: String) {
        self.query = search.trim().to_lowercase();
        self.search = search;
    }

    pub(super) fn matches(&self, title: &str, url: &str) -> bool {
        self.query.is_empty()
            || title.to_lowercase().contains(&self.query)
            || url.to_lowercase().contains(&self.query)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sources(urls: &[&str]) -> BTreeSet<String> {
        urls.iter().map(|url| (*url).to_owned()).collect()
    }

    #[test]
    fn opening_all_and_subset_preserves_the_existing_filter_encoding() {
        let all = SourceFilterDraft::begin(sources(&["a", "b", "c"]), &[]);
        assert_eq!(all.selection(), Selection::All);
        assert_eq!(all.selected_count(), 3);
        assert_eq!(all.apply_value(), Some(vec![]));
        let partial = SourceFilterDraft::begin(
            sources(&["a", "b", "c"]),
            &["c".into(), "a".into(), "missing".into(), "a".into()],
        );
        assert_eq!(partial.selection(), Selection::Partial);
        assert_eq!(partial.apply_value(), Some(vec!["a".into(), "c".into()]));
    }

    #[test]
    fn empty_selection_and_no_available_sources_cannot_apply_as_all() {
        for (available, applied) in
            [(sources(&[]), vec![]), (sources(&["a"]), vec!["removed".into()])]
        {
            let mut draft = SourceFilterDraft::begin(available, &applied);
            assert_eq!(draft.selection(), Selection::Empty);
            assert_eq!(draft.apply_value(), None);
            draft.set_selected("unknown", true);
            assert_eq!(draft.apply_value(), None);
        }
    }

    #[test]
    fn toggle_all_uses_all_sources_even_when_search_hides_some() {
        let mut draft = SourceFilterDraft::begin(sources(&["a", "b", "c"]), &[]);
        draft.set_search("a".into());
        draft.toggle_all();
        assert_eq!(draft.selection(), Selection::Empty);
        assert_eq!(draft.apply_value(), None);
        draft.set_selected("b", true);
        assert_eq!(draft.selection(), Selection::Partial);
        draft.toggle_all();
        assert_eq!(draft.selected_count(), 3);
        assert_eq!(draft.selection(), Selection::All);
    }

    #[test]
    fn search_matches_name_or_url_without_changing_selection() {
        let mut draft = SourceFilterDraft::begin(sources(&["https://A/feed", "b"]), &[]);
        draft.set_search("  TECH  ".into());
        assert_eq!(draft.search(), "  TECH  ");
        assert!(draft.matches("技术 Tech", "https://a/feed"));
        assert!(!draft.matches("其他", "https://a/feed"));
        draft.set_search("HTTPS://a".into());
        assert!(draft.matches("其他", "https://A/feed"));
        draft.set_search("技术".into());
        assert!(draft.matches("技术周刊", "b"));
        assert_eq!(draft.selected_count(), 2);
        assert_eq!(draft.apply_value(), Some(vec![]));
    }

    #[test]
    fn edits_and_cancel_do_not_mutate_the_applied_filter() {
        let applied = vec!["a".to_string()];
        let mut draft = SourceFilterDraft::begin(sources(&["a", "b"]), &applied);
        draft.set_selected("b", true);
        draft.set_search("query".into());
        draft.clear();
        assert_eq!(applied, vec!["a"]);
        assert_eq!(draft.apply_value(), None);
        let reopened = SourceFilterDraft::begin(sources(&["a", "b"]), &applied);
        assert_eq!(reopened.apply_value(), Some(vec!["a".into()]));
        assert_eq!(reopened.search(), "");
    }

    #[test]
    fn source_updates_keep_surviving_choices_but_do_not_select_new_sources() {
        let mut draft = SourceFilterDraft::begin(sources(&["a", "b"]), &[]);
        draft.set_search("query".into());
        draft.reconcile(sources(&["b", "c"]));
        assert!(!draft.is_selected("a"));
        assert!(draft.is_selected("b"));
        assert!(!draft.is_selected("c"));
        assert_eq!(draft.selected_count(), 1);
        assert_eq!(draft.apply_value(), Some(vec!["b".into()]));
        assert_eq!(draft.search(), "query");
        draft.reconcile(sources(&["c"]));
        assert_eq!(draft.selection(), Selection::Empty);
        assert_eq!(draft.apply_value(), None);
    }

    #[test]
    fn removing_unselected_sources_can_make_the_remaining_choice_all() {
        let mut draft = SourceFilterDraft::begin(sources(&["a", "b"]), &["a".into()]);
        draft.reconcile(sources(&["a"]));
        assert_eq!(draft.selection(), Selection::All);
        assert_eq!(draft.apply_value(), Some(vec![]));
        draft.reconcile(sources(&[]));
        assert_eq!(draft.apply_value(), None);
    }
}
