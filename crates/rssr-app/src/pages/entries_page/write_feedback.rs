//! Article-write feedback shared by the page reducer and detached-task delivery.
//! This projection has no task, signal or shell dependency.

use super::intent::EntriesPageIntent;
use rssr_domain::EntryBatchAction;

pub(super) fn selection_applied_message(action: EntryBatchAction, count: u64) -> String {
    let verb = match action {
        EntryBatchAction::MarkRead => "标为已读",
        EntryBatchAction::Archive => "归档",
    };
    format!("已{verb} {count} 篇所选文章。")
}

/// State patches do not produce notices. A receipt's existing error class/tone
/// survives navigation, including an uncertain outcome which must not be retried.
pub(super) fn notice(intent: EntriesPageIntent) -> Option<(String, String)> {
    if let Some(failure) = intent.failure() {
        return Some((failure.message(), failure.tone().to_owned()));
    }
    match intent {
        EntriesPageIntent::SetStatus { message, tone } => Some((message, tone)),
        EntriesPageIntent::SelectionApplied { action, count, .. } => {
            Some((selection_applied_message(action, count), "success".to_owned()))
        }
        _ => None,
    }
}
