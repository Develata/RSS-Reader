use rssr_domain::{EntrySummary, FeedSummary, UserSettings};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppHealth {
    pub ready: bool,
}

#[derive(Debug, Clone)]
pub struct HomeSnapshot {
    pub feeds: Vec<FeedSummary>,
    pub entries: Vec<EntrySummary>,
    pub settings: UserSettings,
}
