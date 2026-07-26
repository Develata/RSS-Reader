use crate::bootstrap::ReaderNavigation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReaderPageLoadedContent {
    pub(crate) title: String,
    pub(crate) body_text: String,
    pub(crate) body_html: Option<String>,
    pub(crate) source: String,
    pub(crate) published_at: String,
    pub(crate) navigation_state: ReaderNavigation,
    pub(crate) is_read: bool,
    pub(crate) is_starred: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReaderPageState {
    /// 当前页面代表哪篇文章。异步加载结果回来时用它判断是否已经过期。
    pub(crate) current_entry_id: i64,
    pub(crate) title: String,
    pub(crate) body_text: String,
    pub(crate) body_html: Option<String>,
    pub(crate) source: String,
    pub(crate) published_at: String,
    pub(crate) navigation_state: ReaderNavigation,
    pub(crate) is_read: bool,
    pub(crate) is_starred: bool,
    pub(crate) reload_tick: u64,
    pub(crate) asset_localization_requested: bool,
    pub(crate) status: String,
    pub(crate) status_tone: String,
    pub(crate) error: Option<String>,
}

impl ReaderPageState {
    pub(crate) fn new() -> Self {
        Self {
            current_entry_id: 0,
            title: "正在加载…".to_string(),
            body_text: String::new(),
            body_html: None,
            source: String::new(),
            published_at: "未知发布时间".to_string(),
            navigation_state: ReaderNavigation::default(),
            is_read: false,
            is_starred: false,
            reload_tick: 0,
            asset_localization_requested: false,
            status: String::new(),
            status_tone: "info".to_string(),
            error: None,
        }
    }

    /// 开始加载 `entry_id`。
    ///
    /// 只有在真的换了文章时才清空状态提示：切换已读/收藏也会走一次重载，
    /// 如果无条件清空，「已标记为已读」这类提示会在一次 DB 读的时间内就被抹掉。
    pub(crate) fn begin_loading(&mut self, entry_id: i64) {
        let switched_entry = self.current_entry_id != entry_id;
        self.current_entry_id = entry_id;
        if switched_entry {
            self.status.clear();
            self.status_tone = "info".to_string();
        }
        self.title = "正在加载…".to_string();
        self.body_text.clear();
        self.body_html = None;
        self.source.clear();
        self.published_at = "未知发布时间".to_string();
        self.navigation_state = ReaderNavigation::default();
        self.is_read = false;
        self.is_starred = false;
        self.asset_localization_requested = false;
        self.error = None;
    }
}
