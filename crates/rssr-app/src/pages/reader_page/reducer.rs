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
        ReaderPageIntent::BeginLoading => state.begin_loading(),
        ReaderPageIntent::ApplyLoadedContent(content) => {
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
        ReaderPageIntent::SetError(error) => state.error = error,
        ReaderPageIntent::BumpReload => state.reload_tick += 1,
    }
}

#[cfg(test)]
mod tests {
    use super::{ReaderPageIntent, ReaderPageState, reduce_reader_page_intent};

    #[test]
    fn marks_asset_localization_request_and_resets_on_reload() {
        let mut state = ReaderPageState::new();

        reduce_reader_page_intent(&mut state, ReaderPageIntent::SetAssetLocalizationRequested);
        assert!(state.asset_localization_requested);

        reduce_reader_page_intent(&mut state, ReaderPageIntent::BeginLoading);
        assert!(!state.asset_localization_requested);
    }
}
