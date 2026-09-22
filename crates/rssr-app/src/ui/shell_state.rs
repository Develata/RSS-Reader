//! Shared shell interaction policy. No routing, network or DOM operations live here.
use crate::router::AppRoute;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NavMode {
    #[default]
    Normal,
    Search,
}

impl NavMode {
    pub(crate) fn toggle(self) -> Self {
        match self {
            Self::Normal => Self::Search,
            Self::Search => Self::Normal,
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
        failed: bool,
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
            Self::Finished { failed: true, .. } => "error",
            Self::Finished { .. } => "finished",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn search_is_a_two_state_toggle() {
        assert_eq!(NavMode::Normal.toggle(), NavMode::Search);
        assert_eq!(NavMode::Search.toggle(), NavMode::Normal);
    }

    #[test]
    fn refresh_deduplicates_until_success_or_failure_finishes() {
        let mut state = ManualRefreshState::default();
        assert!(state.begin());
        for _ in 0..10 {
            assert!(!state.begin());
        }
        for failed in [false, true] {
            state = ManualRefreshState::Finished { message: "result".into(), failed };
            assert!(state.begin());
            assert!(!state.begin());
        }
    }
}
