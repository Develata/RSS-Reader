use crate::ClockPort;
use anyhow::Context;
use rssr_domain::{
    ArchiveFilter, EntryIndexRepository, EntryQuery, FeedRepository, FeedSummary,
    SettingsRepository,
};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FeedsSnapshotOutcome {
    pub feeds: Vec<FeedSummary>,
    pub feed_count: usize,
    pub entry_count: usize,
    pub unread_count: usize,
    pub archived_count: u64,
}

#[derive(Clone)]
pub struct FeedsSnapshotService {
    feed_repository: Arc<dyn FeedRepository>,
    entry_repository: Arc<dyn EntryIndexRepository>,
    settings_repository: Arc<dyn SettingsRepository>,
    clock: Arc<dyn ClockPort>,
}

impl FeedsSnapshotService {
    pub fn new(
        feed_repository: Arc<dyn FeedRepository>,
        entry_repository: Arc<dyn EntryIndexRepository>,
        settings_repository: Arc<dyn SettingsRepository>,
        clock: Arc<dyn ClockPort>,
    ) -> Self {
        Self { feed_repository, entry_repository, settings_repository, clock }
    }

    pub async fn load_snapshot(&self) -> anyhow::Result<FeedsSnapshotOutcome> {
        let feeds = self.feed_repository.list_summaries().await.context("读取订阅失败")?;
        let settings = self.settings_repository.load().await.context("读取归档设置失败")?;
        let archive_filter = match rssr_domain::archive_cutoff_at(
            self.clock.now_utc(),
            settings.archive_after_months,
        ) {
            Some(cutoff) => ArchiveFilter::OnlyArchived { cutoff },
            None => ArchiveFilter::OnlyManual,
        };
        let archived_count = self
            .entry_repository
            .count_entries(&EntryQuery { archive_filter, ..Default::default() })
            .await
            .context("统计归档文章失败")?;
        Ok(summarize(feeds, archived_count))
    }
}

fn summarize(feeds: Vec<FeedSummary>, archived_count: u64) -> FeedsSnapshotOutcome {
    FeedsSnapshotOutcome {
        feed_count: feeds.len(),
        entry_count: feeds.iter().map(|feed| feed.entry_count as usize).sum(),
        unread_count: feeds.iter().map(|feed| feed.unread_count as usize).sum(),
        archived_count,
        feeds,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapshot_counts_all_feeds_and_keeps_archive_count_independent_of_read_state() {
        let feeds = (1..=2)
            .map(|id| FeedSummary {
                id,
                title: format!("Feed {id}"),
                url: format!("https://example.com/{id}.xml"),
                unread_count: id as u32,
                entry_count: 5,
                last_fetched_at: None,
                last_success_at: None,
                fetch_error: None,
            })
            .collect();
        let outcome = summarize(feeds, 4);
        assert_eq!(
            (outcome.feed_count, outcome.entry_count, outcome.unread_count, outcome.archived_count),
            (2, 10, 3, 4)
        );
    }
}
