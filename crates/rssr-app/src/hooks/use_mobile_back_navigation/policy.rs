//! Private return policy. No router, UI state, host calls or running timers.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Navigation {
    Stay,
    History,
    Fallback,
}

impl Navigation {
    pub(super) fn resolve(has_history: bool, has_distinct_fallback: bool) -> Self {
        if has_history {
            Self::History
        } else if has_distinct_fallback {
            Self::Fallback
        } else {
            Self::Stay
        }
    }
}

// The toolbar uses only Navigation; the prompt belongs to Android system back.
#[cfg(any(target_os = "android", test))]
pub(super) use system::*;

#[cfg(any(target_os = "android", test))]
mod system {
    use super::Navigation;
    use std::time::Duration;

    pub(in super::super) const EXIT_CONFIRM_WINDOW: Duration = Duration::from_secs(5);

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(in super::super) struct HintRound(u64);

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(in super::super) enum BackRequest {
        // Includes a saving editor that consumes back without closing.
        Consumed,
        Home,
        Navigate(Navigation),
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(in super::super) enum Event {
        Back(BackRequest),
        HintExpired(HintRound),
        Resumed,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(in super::super) enum Action {
        None,
        Navigate,
        ArmHintTimeout(HintRound),
        Background,
    }

    #[derive(Default)]
    pub(in super::super) struct BackPolicy {
        round: u64,
        hint: Option<HintRound>,
    }

    impl BackPolicy {
        pub(in super::super) fn hint_visible(&self) -> bool {
            self.hint.is_some()
        }

        pub(in super::super) fn handle(&mut self, event: Event) -> Action {
            match event {
                Event::Back(BackRequest::Consumed | BackRequest::Navigate(Navigation::Stay)) => {
                    Action::None
                }
                Event::Back(BackRequest::Navigate(_)) => Action::Navigate,
                Event::Back(BackRequest::Home) => {
                    if self.hint.take().is_some() {
                        Action::Background
                    } else {
                        self.round += 1;
                        let round = HintRound(self.round);
                        self.hint = Some(round);
                        Action::ArmHintTimeout(round)
                    }
                }
                Event::HintExpired(round) => {
                    if self.hint == Some(round) {
                        self.hint = None;
                    }
                    Action::None
                }
                Event::Resumed => {
                    self.hint = None;
                    Action::None
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn show_hint(policy: &mut BackPolicy) -> HintRound {
        let Action::ArmHintTimeout(round) = policy.handle(Event::Back(BackRequest::Home)) else {
            panic!("the first home back must arm a prompt");
        };
        assert!(policy.hint_visible());
        round
    }

    #[test]
    fn history_precedes_fallback_and_an_identical_fallback_is_not_a_target() {
        for (history, fallback, expected) in [
            (true, true, Navigation::History),
            (true, false, Navigation::History),
            (false, true, Navigation::Fallback),
            (false, false, Navigation::Stay),
        ] {
            assert_eq!(Navigation::resolve(history, fallback), expected);
        }
    }

    #[test]
    fn routed_back_never_arms_the_home_prompt_even_without_a_target() {
        let mut policy = BackPolicy::default();
        for (navigation, expected) in [
            (Navigation::History, Action::Navigate),
            (Navigation::Fallback, Action::Navigate),
            (Navigation::Stay, Action::None),
        ] {
            assert_eq!(policy.handle(Event::Back(BackRequest::Navigate(navigation))), expected);
            assert!(!policy.hint_visible());
        }
    }

    #[test]
    fn consumed_back_does_not_navigate_or_change_the_prompt_or_its_deadline() {
        let mut policy = BackPolicy::default();
        assert_eq!(policy.handle(Event::Back(BackRequest::Consumed)), Action::None);
        assert!(!policy.hint_visible());
        let round = show_hint(&mut policy);
        for _ in 0..3 {
            assert_eq!(policy.handle(Event::Back(BackRequest::Consumed)), Action::None);
            assert!(policy.hint_visible());
        }
        assert_eq!(policy.handle(Event::HintExpired(round)), Action::None);
        assert!(!policy.hint_visible());
    }

    #[test]
    fn home_back_shows_then_backgrounds_and_a_third_press_starts_over() {
        let mut policy = BackPolicy::default();
        assert!(!policy.hint_visible());
        let first = show_hint(&mut policy);
        assert_eq!(policy.handle(Event::Back(BackRequest::Home)), Action::Background);
        assert!(!policy.hint_visible());
        assert_ne!(show_hint(&mut policy), first);
    }

    #[test]
    fn expired_prompt_requires_two_new_presses() {
        let mut policy = BackPolicy::default();
        let first = show_hint(&mut policy);
        assert_eq!(policy.handle(Event::HintExpired(first)), Action::None);
        assert!(!policy.hint_visible());
        assert_ne!(show_hint(&mut policy), first);
        assert_eq!(policy.handle(Event::Back(BackRequest::Home)), Action::Background);
        assert!(!policy.hint_visible());
        assert_eq!(EXIT_CONFIRM_WINDOW, std::time::Duration::from_secs(5));
    }

    #[test]
    fn stale_and_repeated_timeouts_cannot_clear_a_later_prompt() {
        let mut policy = BackPolicy::default();
        let first = show_hint(&mut policy);
        policy.handle(Event::HintExpired(first));
        let second = show_hint(&mut policy);
        for _ in 0..3 {
            assert_eq!(policy.handle(Event::HintExpired(first)), Action::None);
            assert!(policy.hint_visible());
        }
        policy.handle(Event::HintExpired(second));
        policy.handle(Event::HintExpired(second));
        assert!(!policy.hint_visible());
    }

    #[test]
    fn resume_clears_the_prompt_without_reusing_its_timer_identity() {
        let mut policy = BackPolicy::default();
        let first = show_hint(&mut policy);
        for _ in 0..3 {
            assert_eq!(policy.handle(Event::Resumed), Action::None);
            assert!(!policy.hint_visible());
        }
        let second = show_hint(&mut policy);
        assert_ne!(first, second);
        policy.handle(Event::HintExpired(first));
        assert!(policy.hint_visible());
        assert_eq!(policy.handle(Event::Back(BackRequest::Home)), Action::Background);
        policy.handle(Event::Resumed);
        policy.handle(Event::HintExpired(second));
        assert!(!policy.hint_visible());
        show_hint(&mut policy);
    }
}
