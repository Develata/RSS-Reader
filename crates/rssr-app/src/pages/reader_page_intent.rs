use super::reader_page_state::ReaderPageLoadedContent;

#[derive(Debug, Clone)]
pub(crate) enum ReaderPageIntent {
    BeginLoading,
    ApplyLoadedContent(ReaderPageLoadedContent),
    SetStatus { message: String, tone: String },
    SetError(Option<String>),
    BumpReload,
}
