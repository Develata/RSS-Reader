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
    /// 只回写被切换的标记，不重载整篇文章。
    ///
    /// 切换已读/收藏此前走 `BumpReload`，而 `begin_loading` 会清空标题、正文、来源、发布时间
    /// 与导航状态——用户每点一次「标已读」，正在读的文章就会整篇闪一下再重绘。
    /// 标记只影响两个布尔字段，用不着整页重载。
    PatchEntryFlags {
        entry_id: i64,
        is_read: Option<bool>,
        is_starred: Option<bool>,
    },
    BumpReload,
}
