//! Shared shell interaction policy. No routing, network or DOM operations live here.
use crate::router::AppRoute;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NavMode {
    #[default]
    Normal,
    Search,
    Collapsed,
}

impl NavMode {
    pub(crate) fn toggle_search(self) -> Self {
        match self {
            Self::Normal => Self::Search,
            Self::Search => Self::Normal,
            Self::Collapsed => Self::Collapsed,
        }
    }

    pub(crate) fn close_search(self) -> Self {
        if self == Self::Search { Self::Normal } else { self }
    }

    pub(crate) fn toggle_collapsed(self) -> Self {
        match self {
            Self::Normal | Self::Search => Self::Collapsed,
            Self::Collapsed => Self::Normal,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum HomeAction {
    Navigate,
    ManualRefresh,
}

pub(crate) fn resolve_home_action(route: &AppRoute) -> HomeAction {
    if matches!(route, AppRoute::EntriesPage {}) {
        HomeAction::ManualRefresh
    } else {
        HomeAction::Navigate
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) enum ManualRefreshState {
    #[default]
    Idle,
    Refreshing,
    Finished {
        message: String,
        /// `info`, `error`, or `unknown` for an outcome that may have happened.
        tone: String,
    },
}

impl ManualRefreshState {
    /// Acquire synchronously before spawning, so repeated events cannot race.
    pub(crate) fn begin(&mut self) -> bool {
        if self.is_refreshing() {
            return false;
        }
        *self = Self::Refreshing;
        true
    }

    pub(crate) fn is_refreshing(&self) -> bool {
        matches!(self, Self::Refreshing)
    }

    pub(crate) fn dismiss_if_current(
        &mut self,
        current_revision: u64,
        completed_revision: u64,
    ) -> bool {
        if current_revision != completed_revision || !matches!(self, Self::Finished { .. }) {
            return false;
        }
        *self = Self::Idle;
        true
    }

    pub(crate) fn label(&self) -> &str {
        match self {
            Self::Idle => "",
            Self::Refreshing => "正在刷新订阅…",
            Self::Finished { message, .. } => message,
        }
    }

    pub(crate) fn phase(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Refreshing => "refreshing",
            Self::Finished { tone, .. } if tone == "error" => "error",
            Self::Finished { tone, .. } if tone == crate::ui::failure::TONE_UNKNOWN => "unknown",
            Self::Finished { .. } => "finished",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::failure::TONE_UNKNOWN;

    #[test]
    fn home_refreshes_only_on_the_global_entries_route() {
        assert_eq!(resolve_home_action(&AppRoute::EntriesPage {}), HomeAction::ManualRefresh);
        for route in [
            AppRoute::StartupPage {},
            AppRoute::FeedsPage {},
            AppRoute::SettingsPage {},
            AppRoute::FeedEntriesPage { feed_id: 7 },
            AppRoute::ReaderPage { entry_id: 9 },
        ] {
            assert_eq!(resolve_home_action(&route), HomeAction::Navigate);
        }
    }

    #[test]
    fn collapsing_search_reopens_normal_and_hidden_search_events_do_not_expand_nav() {
        let search = NavMode::Normal.toggle_search();
        assert_eq!(search, NavMode::Search);
        assert_eq!(search.toggle_search(), NavMode::Normal);
        assert_eq!(search.close_search(), NavMode::Normal);
        let collapsed = search.toggle_collapsed();
        assert_eq!(collapsed, NavMode::Collapsed);
        assert_eq!(NavMode::Normal.toggle_collapsed(), collapsed);
        assert_eq!(collapsed.close_search(), collapsed);
        assert_eq!(collapsed.toggle_search(), collapsed);
        assert_eq!(collapsed.toggle_collapsed(), NavMode::Normal);
    }

    #[test]
    fn refresh_deduplicates_until_success_or_failure_finishes() {
        let mut state = ManualRefreshState::default();
        assert!(state.begin());
        for _ in 0..10 {
            assert!(!state.begin());
        }
        for tone in ["info", "error", "unknown"] {
            state = ManualRefreshState::Finished { message: "result".into(), tone: tone.into() };
            assert!(state.begin());
            assert!(!state.begin());
        }
    }

    /// The bodies of the rules whose selector list holds `selector`.
    fn rule_bodies<'a>(stylesheet: &'a str, selector: &'a str) -> impl Iterator<Item = &'a str> {
        stylesheet.match_indices(selector).filter_map(move |(at, _)| {
            let rest = &stylesheet[at..];
            let open = rest.find('{')?;
            let close = open + rest[open..].find('}')?;
            Some(&rest[open + 1..close])
        })
    }

    /// Every way a refresh can end has its own badge where the result's text is not shown: on the
    /// reader's Home button and on the collapsed navigation's toggle. The phases and symbols are
    /// written out here, not taken from the code under test (reviews C4B-A-R2-01, C4B-A-R3-01,
    /// C4B-A-R3-02). That the elements carry the phase is the browser acceptance's (C4c).
    #[test]
    fn every_finished_refresh_has_its_badge() {
        let stylesheet = include_str!("../../../../assets/styles/shell.css");
        let targets = [
            r#"[data-page="reader"] [data-slot="app-nav-brand"]"#,
            r#"[data-layout="app-nav-shell"][data-state="collapsed"] [data-slot="app-nav-toggle"]"#,
        ];
        let endings =
            [("info", "finished", "✓"), ("error", "error", "!"), (TONE_UNKNOWN, "unknown", "?")];
        for (tone, phase, symbol) in endings {
            let state =
                ManualRefreshState::Finished { message: "结果".into(), tone: tone.into() };
            assert_eq!(state.phase(), phase, "{tone}");
            for target in targets {
                let selector = format!("{target}[data-refresh-state=\"{phase}\"]::after");
                let content = format!("content: \"{symbol}\"");
                assert!(
                    rule_bodies(stylesheet, &selector).any(|body| body.contains(&content)),
                    "{selector}: no {content}"
                );
            }
        }
    }

    #[test]
    fn old_feedback_timeout_cannot_clear_a_new_refresh() {
        let mut state =
            ManualRefreshState::Finished { message: "完成".into(), tone: "info".into() };
        assert!(!state.dismiss_if_current(2, 1));
        assert!(matches!(state, ManualRefreshState::Finished { .. }));
        assert!(state.begin());
        assert!(!state.dismiss_if_current(1, 1));
        state = ManualRefreshState::Finished { message: "新结果".into(), tone: "info".into() };
        assert!(!state.dismiss_if_current(2, 1));
        assert!(state.dismiss_if_current(2, 2));
        assert_eq!(state, ManualRefreshState::Idle);
    }
}
