use super::state::ReaderPageLoadedContent;

#[derive(Debug, Clone)]
pub(crate) enum ReaderPageIntent {
    BeginLoading {
        entry_id: i64,
    },
    /// 带上 `entry_id`：快速翻页时先发起的慢查询可能在后发起的之后落地，
    /// 不校验的话正文会停在上一篇，而路由和「标已读」写的却是当前这篇。
    ApplyLoadedContent {
        entry_id: i64,
        content: ReaderPageLoadedContent,
    },
    SetAssetLocalizationRequested,
    SetStatus {
        message: String,
        tone: String,
    },
    SetError {
        entry_id: i64,
        error: Option<String>,
    },
    BumpReload,
}
