//! Page-local request ownership and refresh feedback rules. The session owns the
//! generation signal and tasks; the reducer remains the only page-state writer.
use std::time::Duration;

use super::intent::FeedsPageIntent;
use crate::ui::{FeedsCommand, ShellCommand, UiCommand};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FeedsRequestPlan {
    ManualRefresh,
    Snapshot { generation: u64 },
    SingleRefresh,
    Other,
}

impl FeedsRequestPlan {
    pub(super) fn for_command(command: &UiCommand, current_generation: u64) -> Self {
        match command {
            UiCommand::Shell(ShellCommand::ManualRefresh) => Self::ManualRefresh,
            UiCommand::Feeds(FeedsCommand::LoadSnapshot) => {
                Self::Snapshot { generation: current_generation.wrapping_add(1) }
            }
            UiCommand::Feeds(FeedsCommand::RefreshFeed { .. }) => Self::SingleRefresh,
            _ => Self::Other,
        }
    }

    pub(super) fn query_generation(self) -> Option<u64> {
        match self {
            Self::Snapshot { generation } => Some(generation),
            _ => None,
        }
    }

    pub(super) fn accepts(self, current_generation: u64) -> bool {
        self.query_generation().is_none_or(|generation| generation == current_generation)
    }
}

pub(super) struct RefreshFeedback {
    pub delay: Duration,
    pub clear: FeedsPageIntent,
}

impl RefreshFeedback {
    /// Capture after the entire accepted result batch has been dispatched. A
    /// newer status may replace this message while the adapter is waiting.
    pub(super) fn capture(revision: u64, message: &str, tone: &str) -> Self {
        Self {
            delay: Duration::from_secs(
                if tone == "error" || tone == crate::ui::failure::TONE_UNKNOWN { 6 } else { 3 },
            ),
            clear: FeedsPageIntent::ClearRefreshStatus { revision, message: message.to_owned() },
        }
    }
}

pub(super) fn refresh_status_matches(
    captured_revision: u64,
    captured_message: &str,
    current_revision: u64,
    current_message: &str,
) -> bool {
    captured_revision == current_revision && captured_message == current_message
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::SettingsCommand;

    #[test]
    fn only_snapshot_reads_allocate_a_new_query_generation() {
        let commands = [
            UiCommand::Feeds(FeedsCommand::AddFeed {
                raw_url: "feed".into(),
                token: 1,
                fallback_site_url: None,
            }),
            UiCommand::Feeds(FeedsCommand::LoadEdit { feed_id: 1, generation: 4 }),
            UiCommand::Feeds(FeedsCommand::SaveEdit {
                generation: 4,
                edit: rssr_domain::FeedEditRequest {
                    feed_id: 1,
                    url: "feed".into(),
                    custom_title: None,
                },
            }),
            UiCommand::Feeds(FeedsCommand::RemoveFeed { feed_id: 1, feed_title: "feed".into() }),
            UiCommand::Feeds(FeedsCommand::ExportConfig),
            UiCommand::Feeds(FeedsCommand::ImportConfig {
                raw: "config".into(),
                expected: rssr_domain::ConfigFingerprint::compute(
                    vec![],
                    &rssr_domain::UserSettings::default(),
                ),
            }),
            UiCommand::Feeds(FeedsCommand::ExportOpml),
            UiCommand::Feeds(FeedsCommand::ImportOpml { raw: "opml".into() }),
            UiCommand::Settings(SettingsCommand::Load),
        ];
        for command in commands {
            let plan = FeedsRequestPlan::for_command(&command, 7);
            assert_eq!(plan, FeedsRequestPlan::Other, "{command:?}");
            assert_eq!(plan.query_generation(), None);
            assert!(plan.accepts(8), "new snapshots must not discard command completion");
        }
        let snapshot =
            FeedsRequestPlan::for_command(&UiCommand::Feeds(FeedsCommand::LoadSnapshot), 7);
        assert_eq!(snapshot.query_generation(), Some(8));
        assert!(!snapshot.accepts(7));
        assert!(snapshot.accepts(8));
        assert!(!snapshot.accepts(9));
    }

    #[test]
    fn snapshot_generation_keeps_wrapping_semantics() {
        let plan =
            FeedsRequestPlan::for_command(&UiCommand::Feeds(FeedsCommand::LoadSnapshot), u64::MAX);
        assert_eq!(plan.query_generation(), Some(0));
        assert!(plan.accepts(0));
        assert!(!plan.accepts(u64::MAX));
    }

    #[test]
    fn shell_and_single_refresh_have_different_owners_without_query_tokens() {
        let manual =
            FeedsRequestPlan::for_command(&UiCommand::Shell(ShellCommand::ManualRefresh), 5);
        let single = FeedsRequestPlan::for_command(
            &UiCommand::Feeds(FeedsCommand::RefreshFeed { feed_id: 42, feed_title: "feed".into() }),
            5,
        );
        assert_eq!(manual, FeedsRequestPlan::ManualRefresh);
        assert_eq!(single, FeedsRequestPlan::SingleRefresh);
        assert_eq!(manual.query_generation(), None);
        assert_eq!(single.query_generation(), None);
        assert!(single.accepts(6));
    }

    #[test]
    fn feedback_duration_preserves_error_and_unknown_tones() {
        for (tone, seconds) in [
            ("error", 6),
            (crate::ui::failure::TONE_UNKNOWN, 6),
            ("info", 3),
            ("success", 3),
            ("Error", 3),
            ("", 3),
        ] {
            assert_eq!(
                RefreshFeedback::capture(1, "status", tone).delay,
                Duration::from_secs(seconds)
            );
        }
    }

    #[test]
    fn feedback_captures_its_message_and_revision_including_empty_messages() {
        for initial in ["saved", ""] {
            let mut current = initial.to_string();
            let feedback = RefreshFeedback::capture(u64::MAX, &current, "info");
            current.push_str("newer status");
            let FeedsPageIntent::ClearRefreshStatus { revision, message } = feedback.clear else {
                panic!("expected delayed clear")
            };
            assert_eq!(revision, u64::MAX);
            assert_eq!(message, initial);
            assert_ne!(message, current);
        }
    }

    #[test]
    fn clearing_requires_both_revision_and_message_identity() {
        assert!(refresh_status_matches(7, "same", 7, "same"));
        assert!(!refresh_status_matches(7, "same", 8, "same"));
        assert!(!refresh_status_matches(7, "old", 7, "new"));
        assert!(!refresh_status_matches(u64::MAX, "same", 0, "same"));
        assert!(refresh_status_matches(0, "", 0, ""));
    }
}
