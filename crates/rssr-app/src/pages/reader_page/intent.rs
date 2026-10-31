use super::state::{ReaderFlag, ReaderPageLoadedContent};
use crate::ui::failure::UiFailure;

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
    /// Reading the entry failed (C4b item 1); shown like [`Self::SetError`].
    LoadFailed {
        entry_id: i64,
        failure: UiFailure,
    },
    /// A backend call failed (C4b item 1).
    Failed(UiFailure),
    /// A read-only refresh failed; a later successful read can retire this feedback.
    RereadFailed {
        entry_id: i64,
        failure: UiFailure,
    },
    /// One in-flight write per flag in this visit; read and starred remain independent.
    FlagWriteStarted(ReaderFlag),
    FlagWriteFinished(ReaderFlag),
    /// The read-only refresh's answer (C4b item 2): the flags and the neighbours. The body, the
    /// title and what the page did with them (image localization, position) stay.
    ApplyReread {
        entry_id: i64,
        is_read: bool,
        is_starred: bool,
        navigation_state: rssr_domain::EntryNavigation,
    },
    /// The read-only refresh found the entry gone; what is shown stays readable.
    RereadMissing {
        entry_id: i64,
    },
    /// A read-only refresh was issued: earlier ones are superseded (review C4B-B-01).
    RereadIssued,
    /// A read-only refresh is wanted after the load on its way (`true`), or was taken.
    RereadWanted(bool),
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
}

impl ReaderPageIntent {
    /// The backend failure this intent carries.
    pub(crate) fn failure(&self) -> Option<&UiFailure> {
        match self {
            Self::LoadFailed { failure, .. }
            | Self::RereadFailed { failure, .. }
            | Self::Failed(failure) => Some(failure),
            _ => None,
        }
    }
}
