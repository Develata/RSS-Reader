//! Status text for setting an entry's read or starred flag, shared by the entry list and the
//! reader. The subject names the entry: its title in the list, "当前文章" in the reader.

use rssr_application::EntryFlagOutcome;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Flag {
    Read,
    Starred,
}

/// A missing entry is shown like a failure (the reader announces other results only to screen
/// readers): the request could not take effect, though nothing is wrong.
pub(super) fn flag_tone(outcome: EntryFlagOutcome) -> &'static str {
    match outcome {
        EntryFlagOutcome::Set { .. } => "info",
        EntryFlagOutcome::Missing => "error",
    }
}

pub(super) fn flag_message(flag: Flag, subject: &str, outcome: EntryFlagOutcome) -> String {
    let EntryFlagOutcome::Set { value, changed } = outcome else {
        return format!("{subject}已不存在，可能已被刷新清理或订阅已删除。");
    };
    match (flag, value, changed) {
        (Flag::Read, true, true) => format!("已将{subject}标记为已读。"),
        (Flag::Read, false, true) => format!("已将{subject}标记为未读。"),
        (Flag::Read, true, false) => format!("{subject}已是已读。"),
        (Flag::Read, false, false) => format!("{subject}已是未读。"),
        (Flag::Starred, true, true) => format!("已收藏{subject}。"),
        (Flag::Starred, false, true) => format!("已取消收藏{subject}。"),
        (Flag::Starred, true, false) => format!("{subject}已在收藏中。"),
        (Flag::Starred, false, false) => format!("{subject}本来就未收藏。"),
    }
}

#[cfg(test)]
mod tests {
    use rssr_application::EntryFlagOutcome;

    use super::{Flag, flag_message};

    #[test]
    fn each_outcome_has_its_own_text() {
        let set = |value, changed| EntryFlagOutcome::Set { value, changed };
        assert_eq!(flag_message(Flag::Read, "《A》", set(true, true)), "已将《A》标记为已读。");
        assert_eq!(flag_message(Flag::Read, "《A》", set(false, false)), "《A》已是未读。");
        assert_eq!(
            flag_message(Flag::Starred, "当前文章", set(false, true)),
            "已取消收藏当前文章。"
        );
        assert_eq!(
            flag_message(Flag::Starred, "当前文章", set(true, false)),
            "当前文章已在收藏中。"
        );
        assert_eq!(
            flag_message(Flag::Read, "《A》", EntryFlagOutcome::Missing),
            "《A》已不存在，可能已被刷新清理或订阅已删除。"
        );
    }
}
