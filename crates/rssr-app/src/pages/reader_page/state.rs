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
    /// 每次加载递增：A → B → A 后，第一次 A 的结果也必须视为过期。
    pub(crate) load_generation: u64,
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
            load_generation: 0,
            asset_localization_requested: false,
            status: String::new(),
            status_tone: "info".to_string(),
            error: None,
        }
    }

    /// 开始加载 `entry_id`。
    ///
    /// 同一篇的显式重载保留提示，切换文章时清空提示。
    pub(crate) fn begin_loading(&mut self, entry_id: i64) {
        let switched_entry = self.current_entry_id != entry_id;
        self.current_entry_id = entry_id;
        self.load_generation = self.load_generation.wrapping_add(1);
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
