use dioxus::prelude::*;

use super::{intent::ReaderPageIntent, state::ReaderPageState};

pub(crate) fn dispatch_reader_page_intent(
    mut state: Signal<ReaderPageState>,
    intent: ReaderPageIntent,
) {
    state.with_mut(|state| reduce_reader_page_intent(state, intent));
}

pub(crate) fn reduce_reader_page_intent(state: &mut ReaderPageState, intent: ReaderPageIntent) {
    match intent {
        ReaderPageIntent::BeginLoading { entry_id } => state.begin_loading(entry_id),
        ReaderPageIntent::ApplyLoadedContent { entry_id, content } => {
            if entry_id != state.current_entry_id {
                return;
            }
            state.title = content.title;
            state.body_text = content.body_text;
            state.body_html = content.body_html;
            state.source = content.source;
            state.published_at = content.published_at;
            state.navigation_state = content.navigation_state;
            state.is_read = content.is_read;
            state.is_starred = content.is_starred;
            state.error = None;
        }
        ReaderPageIntent::SetAssetLocalizationRequested => {
            state.asset_localization_requested = true;
        }
        ReaderPageIntent::SetStatus { message, tone } => {
            state.status = message;
            state.status_tone = tone;
        }
        ReaderPageIntent::SetError { entry_id, error } => {
            if entry_id != state.current_entry_id {
                return;
            }
            state.error = error;
        }
        ReaderPageIntent::BumpReload => state.reload_tick += 1,
    }
}

#[cfg(test)]
mod tests {
    use super::{ReaderPageIntent, ReaderPageState, reduce_reader_page_intent};
    use crate::pages::reader_page::state::ReaderPageLoadedContent;

    fn loaded_content(title: &str) -> ReaderPageLoadedContent {
        ReaderPageLoadedContent {
            title: title.to_string(),
            body_text: format!("{title} body"),
            body_html: None,
            source: "https://example.com/post".to_string(),
            published_at: "2026-07-26 10:00 UTC".to_string(),
            navigation_state: Default::default(),
            is_read: false,
            is_starred: false,
        }
    }

    #[test]
    fn discards_load_result_that_belongs_to_a_previous_entry() {
        let mut state = ReaderPageState::new();

        // 已经翻到第 2 篇。
        reduce_reader_page_intent(&mut state, ReaderPageIntent::BeginLoading { entry_id: 2 });
        reduce_reader_page_intent(
            &mut state,
            ReaderPageIntent::ApplyLoadedContent { entry_id: 2, content: loaded_content("Second") },
        );
        assert_eq!(state.title, "Second");

        // 第 1 篇的慢查询迟到落地：必须被丢弃，否则正文会和「标已读」写入的文章不一致。
        reduce_reader_page_intent(
            &mut state,
            ReaderPageIntent::ApplyLoadedContent { entry_id: 1, content: loaded_content("First") },
        );
        assert_eq!(state.title, "Second");

        reduce_reader_page_intent(
            &mut state,
            ReaderPageIntent::SetError { entry_id: 1, error: Some("stale".to_string()) },
        );
        assert_eq!(state.error, None);
    }

    #[test]
    fn reloading_the_same_entry_keeps_the_status_message() {
        let mut state = ReaderPageState::new();
        reduce_reader_page_intent(&mut state, ReaderPageIntent::BeginLoading { entry_id: 7 });
        reduce_reader_page_intent(
            &mut state,
            ReaderPageIntent::SetStatus {
                message: "已将当前文章标记为已读。".to_string(),
                tone: "info".to_string(),
            },
        );

        // 切换已读会触发同一篇文章的重载；提示不应该被这次重载抹掉。
        reduce_reader_page_intent(&mut state, ReaderPageIntent::BeginLoading { entry_id: 7 });
        assert_eq!(state.status, "已将当前文章标记为已读。");

        // 换到另一篇时才清空。
        reduce_reader_page_intent(&mut state, ReaderPageIntent::BeginLoading { entry_id: 8 });
        assert!(state.status.is_empty());
    }

    #[test]
    fn marks_asset_localization_request_and_resets_on_reload() {
        let mut state = ReaderPageState::new();

        reduce_reader_page_intent(&mut state, ReaderPageIntent::SetAssetLocalizationRequested);
        assert!(state.asset_localization_requested);

        reduce_reader_page_intent(&mut state, ReaderPageIntent::BeginLoading { entry_id: 1 });
        assert!(!state.asset_localization_requested);
    }
}
