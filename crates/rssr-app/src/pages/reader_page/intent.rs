use super::state::ReaderPageLoadedContent;

#[derive(Debug, Clone)]
pub(crate) enum ReaderPageIntent {
    BeginLoading,
    ApplyLoadedContent(ReaderPageLoadedContent),
    SetStatus { message: String, tone: String },
    SetError(Option<String>),
    BumpReload,
}
