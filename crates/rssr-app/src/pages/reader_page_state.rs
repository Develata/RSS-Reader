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
    pub(crate) title: String,
    pub(crate) body_text: String,
    pub(crate) body_html: Option<String>,
    pub(crate) source: String,
    pub(crate) published_at: String,
    pub(crate) navigation_state: ReaderNavigation,
    pub(crate) is_read: bool,
    pub(crate) is_starred: bool,
    pub(crate) reload_tick: u64,
    pub(crate) status: String,
    pub(crate) status_tone: String,
    pub(crate) error: Option<String>,
}

impl ReaderPageState {
    pub(crate) fn new() -> Self {
        Self {
            title: "正在加载…".to_string(),
            body_text: String::new(),
            body_html: None,
            source: String::new(),
            published_at: "未知发布时间".to_string(),
            navigation_state: ReaderNavigation::default(),
            is_read: false,
            is_starred: false,
            reload_tick: 0,
            status: String::new(),
            status_tone: "info".to_string(),
            error: None,
        }
    }

    pub(crate) fn begin_loading(&mut self) {
        self.title = "正在加载…".to_string();
        self.body_text.clear();
        self.body_html = None;
        self.source.clear();
        self.published_at = "未知发布时间".to_string();
        self.navigation_state = ReaderNavigation::default();
        self.is_read = false;
        self.is_starred = false;
        self.status.clear();
        self.status_tone = "info".to_string();
        self.error = None;
    }
}
