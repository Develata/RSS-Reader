#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedSummary {
    pub id: i64,
    pub title: String,
    pub unread_count: u32,
}

