use crate::dto::AppHealth;

pub struct FeedService;

impl FeedService {
    pub fn health(&self) -> AppHealth {
        AppHealth { ready: true }
    }
}

