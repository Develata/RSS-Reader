use rssr_domain::EntryNavigation as ReaderNavigation;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReaderFlag {
    Read,
    Starred,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReaderPageLoadedContent {
    pub(crate) title: String,
    pub(crate) body_text: String,
    pub(crate) body_html: Option<String>,
    pub(crate) source: String,
    pub(crate) author: Option<String>,
    pub(crate) original_url: Option<String>,
    pub(crate) published_at: String,
    pub(crate) navigation_state: ReaderNavigation,
    pub(crate) is_read: bool,
    pub(crate) is_starred: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReaderPageState {
    pub(crate) content_loaded: bool,
    /// 当前页面代表哪篇文章。异步加载结果回来时用它判断是否已经过期。
    pub(crate) current_entry_id: i64,
    pub(crate) title: String,
    // 页面快照与按钮闭包共享不可变正文，切换标记不深拷贝整篇文章。
    pub(crate) body_text: Arc<str>,
    pub(crate) body_html: Option<Arc<str>>,
    pub(crate) source: String,
    pub(crate) author: Option<String>,
    pub(crate) original_url: Option<String>,
    pub(crate) published_at: String,
    pub(crate) navigation_state: ReaderNavigation,
    pub(crate) is_read: bool,
    pub(crate) is_starred: bool,
    /// 每次加载递增：A → B → A 后，第一次 A 的结果也必须视为过期。
    pub(crate) load_generation: u64,
    pub(crate) read_write_pending: bool,
    pub(crate) starred_write_pending: bool,
    /// Bumped by every read-only refresh issued and every flag change the page applies: an
    /// earlier refresh's answer is dropped, since it may have read before the later one or before
    /// the write (C4b item 2; review C4B-B-01).
    pub(crate) reread_generation: u64,
    /// A read-only refresh was asked for while the entry's load was on its way: it follows the
    /// load (review C4B-B-R3-03).
    pub(crate) reread_wanted: bool,
    pub(crate) asset_localization_requested: bool,
    pub(crate) status: String,
    pub(crate) status_tone: String,
    pub(crate) error: Option<String>,
    /// The tone `error` is shown with: a missing entry or a failure is an error, an unknown
    /// outcome is not (review C4B-A-R5-01).
    pub(crate) error_tone: &'static str,
}

impl ReaderPageState {
    pub(crate) fn load_state(&self, entry_id: i64) -> &'static str {
        if self.current_entry_id != entry_id {
            "pending"
        } else if self.error.is_some() {
            "error"
        } else if self.content_loaded {
            "loaded"
        } else {
            "pending"
        }
    }

    pub(crate) fn new() -> Self {
        Self {
            content_loaded: false,
            current_entry_id: 0,
            title: "正在加载…".to_string(),
            body_text: Arc::from(""),
            body_html: None,
            source: String::new(),
            author: None,
            original_url: None,
            published_at: "未知发布时间".to_string(),
            navigation_state: ReaderNavigation::default(),
            is_read: false,
            is_starred: false,
            load_generation: 0,
            read_write_pending: false,
            starred_write_pending: false,
            reread_generation: 0,
            reread_wanted: false,
            asset_localization_requested: false,
            status: String::new(),
            status_tone: "info".to_string(),
            error: None,
            error_tone: "error",
        }
    }

    /// 屏幕上的已读/收藏是否就是 `entry_id` 存储里的值，可以据此切换：加载中不是（标记先被
    /// 清成 false，切换之后才到的加载结果还会把它改回去），加载失败也不是。
    pub(crate) fn flags_known(&self, entry_id: i64) -> bool {
        self.current_entry_id == entry_id && self.content_loaded && self.error.is_none()
    }

    pub(crate) fn flag_write_pending(&self, flag: ReaderFlag) -> bool {
        match flag {
            ReaderFlag::Read => self.read_write_pending,
            ReaderFlag::Starred => self.starred_write_pending,
        }
    }

    pub(crate) fn can_write_flag(&self, entry_id: i64, flag: ReaderFlag) -> bool {
        self.flags_known(entry_id) && !self.flag_write_pending(flag)
    }

    pub(super) fn set_flag_write_pending(&mut self, flag: ReaderFlag, pending: bool) {
        match flag {
            ReaderFlag::Read => self.read_write_pending = pending,
            ReaderFlag::Starred => self.starred_write_pending = pending,
        }
    }

    /// 开始加载 `entry_id`。
    ///
    /// 同一篇的显式重载保留提示，切换文章时清空提示。
    pub(crate) fn begin_loading(&mut self, entry_id: i64) {
        self.content_loaded = false;
        let switched_entry = self.current_entry_id != entry_id;
        self.current_entry_id = entry_id;
        self.load_generation = self.load_generation.wrapping_add(1);
        self.read_write_pending = false;
        self.starred_write_pending = false;
        if switched_entry {
            self.status.clear();
            self.status_tone = "info".to_string();
        }
        self.title = "正在加载…".to_string();
        self.body_text = Arc::from("");
        self.body_html = None;
        self.source.clear();
        self.author = None;
        self.original_url = None;
        self.published_at = "未知发布时间".to_string();
        self.navigation_state = ReaderNavigation::default();
        self.is_read = false;
        self.is_starred = false;
        self.asset_localization_requested = false;
        self.error = None;
        // A new load reads everything a pending refresh would.
        self.reread_wanted = false;
    }
}

#[cfg(test)]
mod tests {
    use super::ReaderPageState;

    #[test]
    fn load_state_tracks_current_content_and_clears_failure_on_retry() {
        let mut state = ReaderPageState::new();
        assert_eq!(state.load_state(1), "pending");
        state.begin_loading(1);
        assert_eq!(state.load_state(1), "pending");
        state.content_loaded = true;
        assert_eq!(state.load_state(1), "loaded");
        assert_eq!(state.load_state(2), "pending");

        state.error = Some("load failed".into());
        assert_eq!(state.load_state(1), "error");
        assert_eq!(state.load_state(2), "pending");
        state.begin_loading(1);
        assert_eq!(state.load_state(1), "pending");
        state.content_loaded = true;
        assert_eq!(state.load_state(1), "loaded");
    }
}
