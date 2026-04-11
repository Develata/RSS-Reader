use std::sync::Arc;

use anyhow::{Context, Result};
use time::OffsetDateTime;
use url::Url;

#[cfg(not(target_arch = "wasm32"))]
use tokio::task::JoinSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefreshTarget {
    pub feed_id: i64,
    pub url: Url,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RefreshHttpMetadata {
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedFeedUpdate {
    pub title: Option<String>,
    pub site_url: Option<Url>,
    pub description: Option<String>,
    pub entries: Vec<ParsedEntryData>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedEntryData {
    pub external_id: String,
    pub dedup_key: String,
    pub url: Option<Url>,
    pub title: String,
    pub author: Option<String>,
    pub summary: Option<String>,
    pub content_html: Option<String>,
    pub content_text: Option<String>,
    pub published_at: Option<OffsetDateTime>,
    pub updated_at_source: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedRefreshUpdate {
    pub metadata: RefreshHttpMetadata,
    pub feed: ParsedFeedUpdate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefreshFailure {
    pub message: String,
    pub metadata: Option<RefreshHttpMetadata>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeedRefreshSourceOutput {
    NotModified(RefreshHttpMetadata),
    Updated(FeedRefreshUpdate),
    Failed(RefreshFailure),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefreshCommit {
    NotModified { metadata: RefreshHttpMetadata },
    Updated { update: FeedRefreshUpdate },
    Failed { failure: RefreshFailure },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefreshAllInput {
    pub max_concurrency: usize,
}

impl Default for RefreshAllInput {
    fn default() -> Self {
        Self { max_concurrency: 1 }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefreshFeedResult {
    NotModified,
    Updated { entry_count: usize, localization_entries: Vec<RefreshLocalizedEntry> },
    Failed { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefreshLocalizedEntry {
    pub dedup_key: String,
    pub url: Option<Url>,
    pub title: String,
    pub content_html: String,
    pub content_text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefreshFeedOutcome {
    pub feed_id: i64,
    pub url: String,
    pub result: RefreshFeedResult,
}

impl RefreshFeedOutcome {
    pub fn is_success(&self) -> bool {
        !matches!(self.result, RefreshFeedResult::Failed { .. })
    }

    pub fn failure_message(&self) -> Option<&str> {
        match &self.result {
            RefreshFeedResult::Failed { message } => Some(message),
            _ => None,
        }
    }

    pub fn failure_summary(&self) -> Option<RefreshFeedFailureSummary> {
        self.failure_message().map(|message| RefreshFeedFailureSummary {
            feed_id: self.feed_id,
            url: self.url.clone(),
            message: message.to_string(),
        })
    }

    pub fn failure_line(&self) -> Option<String> {
        self.failure_summary().map(|failure| failure.display_line())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefreshFeedFailureSummary {
    pub feed_id: i64,
    pub url: String,
    pub message: String,
}

impl RefreshFeedFailureSummary {
    pub fn display_line(&self) -> String {
        format!("{}: {}", self.url, self.message)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RefreshAllOutcome {
    pub feeds: Vec<RefreshFeedOutcome>,
}

impl RefreshAllOutcome {
    pub fn summary(&self) -> RefreshAllSummary {
        let mut summary =
            RefreshAllSummary { total_count: self.feeds.len(), ..RefreshAllSummary::default() };

        for outcome in &self.feeds {
            match &outcome.result {
                RefreshFeedResult::Updated { .. } => summary.updated_count += 1,
                RefreshFeedResult::NotModified => summary.not_modified_count += 1,
                RefreshFeedResult::Failed { .. } => {
                    summary.failed_count += 1;
                    if let Some(failure) = outcome.failure_summary() {
                        summary.failures.push(failure);
                    }
                }
            }
        }

        summary
    }

    pub fn has_failures(&self) -> bool {
        self.summary().has_failures()
    }

    pub fn updated_count(&self) -> usize {
        self.summary().updated_count
    }

    pub fn not_modified_count(&self) -> usize {
        self.summary().not_modified_count
    }

    pub fn failures(&self) -> Vec<&RefreshFeedOutcome> {
        self.feeds.iter().filter(|outcome| outcome.failure_message().is_some()).collect()
    }

    pub fn failure_summaries(&self) -> Vec<RefreshFeedFailureSummary> {
        self.summary().failures
    }

    pub fn joined_failure_lines(&self) -> Option<String> {
        self.summary().joined_failure_lines()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RefreshAllSummary {
    pub total_count: usize,
    pub updated_count: usize,
    pub not_modified_count: usize,
    pub failed_count: usize,
    pub failures: Vec<RefreshFeedFailureSummary>,
}

impl RefreshAllSummary {
    pub fn has_failures(&self) -> bool {
        self.failed_count > 0
    }

    pub fn joined_failure_lines(&self) -> Option<String> {
        if self.failures.is_empty() {
            None
        } else {
            Some(
                self.failures
                    .iter()
                    .map(RefreshFeedFailureSummary::display_line)
                    .collect::<Vec<_>>()
                    .join(" | "),
            )
        }
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
pub trait FeedRefreshSourcePort: Send + Sync {
    async fn refresh(&self, target: &RefreshTarget) -> Result<FeedRefreshSourceOutput>;
}

#[async_trait::async_trait]
pub trait RefreshStorePort: Send + Sync {
    async fn list_targets(&self) -> Result<Vec<RefreshTarget>>;
    async fn get_target(&self, feed_id: i64) -> Result<Option<RefreshTarget>>;
    async fn commit(&self, feed_id: i64, commit: RefreshCommit) -> Result<()>;
}

#[derive(Clone)]
pub struct RefreshService {
    source: Arc<dyn FeedRefreshSourcePort>,
    store: Arc<dyn RefreshStorePort>,
}

impl RefreshService {
    pub fn new(source: Arc<dyn FeedRefreshSourcePort>, store: Arc<dyn RefreshStorePort>) -> Self {
        Self { source, store }
    }

    pub async fn refresh_feed(&self, feed_id: i64) -> Result<RefreshFeedOutcome> {
        let target = self.store.get_target(feed_id).await?.context("订阅不存在")?;
        self.refresh_target(target).await
    }

    pub async fn refresh_all(&self, input: RefreshAllInput) -> Result<RefreshAllOutcome> {
        let targets = self.store.list_targets().await.context("读取订阅列表失败")?;

        #[cfg(target_arch = "wasm32")]
        {
            let _ = input;
            let mut outcomes = Vec::with_capacity(targets.len());
            for target in targets {
                outcomes.push(self.refresh_target(target).await?);
            }
            return Ok(RefreshAllOutcome { feeds: outcomes });
        }

        #[cfg(not(target_arch = "wasm32"))]
        let max_concurrency = input.max_concurrency.max(1);

        #[cfg(not(target_arch = "wasm32"))]
        if max_concurrency == 1 {
            let mut outcomes = Vec::with_capacity(targets.len());
            for target in targets {
                outcomes.push(self.refresh_target(target).await?);
            }
            return Ok(RefreshAllOutcome { feeds: outcomes });
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let mut outcomes = Vec::with_capacity(targets.len());
            let mut target_iter = targets.into_iter();
            let mut in_flight = JoinSet::new();

            loop {
                while in_flight.len() < max_concurrency {
                    let Some(target) = target_iter.next() else {
                        break;
                    };
                    let service = self.clone();
                    in_flight.spawn(async move { service.refresh_target(target).await });
                }

                let Some(result) = in_flight.join_next().await else {
                    break;
                };

                match result {
                    Ok(outcome) => outcomes.push(outcome?),
                    Err(error) => {
                        return Err(anyhow::Error::new(error).context("后台刷新任务意外结束"));
                    }
                }
            }

            Ok(RefreshAllOutcome { feeds: outcomes })
        }
    }

    async fn refresh_target(&self, target: RefreshTarget) -> Result<RefreshFeedOutcome> {
        let source_output = match self.source.refresh(&target).await {
            Ok(output) => output,
            Err(error) => FeedRefreshSourceOutput::Failed(RefreshFailure {
                message: error.to_string(),
                metadata: None,
            }),
        };

        match source_output {
            FeedRefreshSourceOutput::NotModified(metadata) => {
                self.store
                    .commit(
                        target.feed_id,
                        RefreshCommit::NotModified { metadata: metadata.clone() },
                    )
                    .await?;
                Ok(RefreshFeedOutcome {
                    feed_id: target.feed_id,
                    url: target.url.to_string(),
                    result: RefreshFeedResult::NotModified,
                })
            }
            FeedRefreshSourceOutput::Updated(update) => {
                let entry_count = update.feed.entries.len();
                let localization_entries = build_localization_entries(&update.feed.entries);
                self.store.commit(target.feed_id, RefreshCommit::Updated { update }).await?;
                Ok(RefreshFeedOutcome {
                    feed_id: target.feed_id,
                    url: target.url.to_string(),
                    result: RefreshFeedResult::Updated { entry_count, localization_entries },
                })
            }
            FeedRefreshSourceOutput::Failed(failure) => {
                self.store
                    .commit(target.feed_id, RefreshCommit::Failed { failure: failure.clone() })
                    .await?;
                Ok(RefreshFeedOutcome {
                    feed_id: target.feed_id,
                    url: target.url.to_string(),
                    result: RefreshFeedResult::Failed { message: failure.message },
                })
            }
        }
    }
}

fn build_localization_entries(entries: &[ParsedEntryData]) -> Vec<RefreshLocalizedEntry> {
    entries
        .iter()
        .filter_map(|entry| {
            entry.content_html.as_ref().map(|content_html| RefreshLocalizedEntry {
                dedup_key: entry.dedup_key.clone(),
                url: entry.url.clone(),
                title: entry.title.clone(),
                content_html: content_html.clone(),
                content_text: entry.content_text.clone(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use anyhow::Result;
    use url::Url;

    use super::{
        FeedRefreshSourceOutput, FeedRefreshSourcePort, FeedRefreshUpdate, ParsedEntryData,
        ParsedFeedUpdate, RefreshAllInput, RefreshCommit, RefreshFailure, RefreshHttpMetadata,
        RefreshService, RefreshStorePort, RefreshTarget,
    };

    struct SourceStub {
        outputs: Mutex<Vec<FeedRefreshSourceOutput>>,
    }

    #[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
    #[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
    impl FeedRefreshSourcePort for SourceStub {
        async fn refresh(&self, _target: &RefreshTarget) -> Result<FeedRefreshSourceOutput> {
            Ok(self.outputs.lock().expect("lock outputs").remove(0))
        }
    }

    struct StoreStub {
        targets: Vec<RefreshTarget>,
        commits: Mutex<Vec<(i64, RefreshCommit)>>,
    }

    #[async_trait::async_trait]
    impl RefreshStorePort for StoreStub {
        async fn list_targets(&self) -> Result<Vec<RefreshTarget>> {
            Ok(self.targets.clone())
        }

        async fn get_target(&self, feed_id: i64) -> Result<Option<RefreshTarget>> {
            Ok(self.targets.iter().find(|target| target.feed_id == feed_id).cloned())
        }

        async fn commit(&self, feed_id: i64, commit: RefreshCommit) -> Result<()> {
            self.commits.lock().expect("lock commits").push((feed_id, commit));
            Ok(())
        }
    }

    fn sample_target(feed_id: i64, url: &str) -> RefreshTarget {
        RefreshTarget {
            feed_id,
            url: Url::parse(url).expect("valid url"),
            etag: Some("etag".to_string()),
            last_modified: Some("last-modified".to_string()),
        }
    }

    fn sample_entry() -> ParsedEntryData {
        ParsedEntryData {
            external_id: "entry-1".to_string(),
            dedup_key: "entry-1".to_string(),
            url: Some(Url::parse("https://example.com/entry-1").expect("valid url")),
            title: "Entry".to_string(),
            author: None,
            summary: Some("summary".to_string()),
            content_html: Some("<p>summary</p>".to_string()),
            content_text: Some("summary".to_string()),
            published_at: None,
            updated_at_source: None,
        }
    }

    fn sample_entry_without_html() -> ParsedEntryData {
        ParsedEntryData {
            external_id: "entry-2".to_string(),
            dedup_key: "entry-2".to_string(),
            url: Some(Url::parse("https://example.com/entry-2").expect("valid url")),
            title: "Entry 2".to_string(),
            author: None,
            summary: Some("summary 2".to_string()),
            content_html: None,
            content_text: Some("summary 2".to_string()),
            published_at: None,
            updated_at_source: None,
        }
    }

    #[test]
    fn refresh_feed_failure_summary_keeps_feed_identity() {
        let outcome = super::RefreshFeedOutcome {
            feed_id: 9,
            url: "https://example.com/feed.xml".to_string(),
            result: super::RefreshFeedResult::Failed { message: "boom".to_string() },
        };

        assert!(!outcome.is_success());
        assert_eq!(outcome.failure_message(), Some("boom"));
        assert_eq!(outcome.failure_line(), Some("https://example.com/feed.xml: boom".to_string()));
        assert_eq!(
            outcome.failure_summary(),
            Some(super::RefreshFeedFailureSummary {
                feed_id: 9,
                url: "https://example.com/feed.xml".to_string(),
                message: "boom".to_string(),
            })
        );
    }

    #[test]
    fn refresh_all_summary_counts_results_and_formats_failures() {
        let outcome = super::RefreshAllOutcome {
            feeds: vec![
                super::RefreshFeedOutcome {
                    feed_id: 1,
                    url: "https://example.com/one.xml".to_string(),
                    result: super::RefreshFeedResult::Updated {
                        entry_count: 2,
                        localization_entries: Vec::new(),
                    },
                },
                super::RefreshFeedOutcome {
                    feed_id: 2,
                    url: "https://example.com/two.xml".to_string(),
                    result: super::RefreshFeedResult::NotModified,
                },
                super::RefreshFeedOutcome {
                    feed_id: 3,
                    url: "https://example.com/three.xml".to_string(),
                    result: super::RefreshFeedResult::Failed { message: "boom".to_string() },
                },
            ],
        };

        let summary = outcome.summary();

        assert_eq!(summary.total_count, 3);
        assert_eq!(summary.updated_count, 1);
        assert_eq!(summary.not_modified_count, 1);
        assert_eq!(summary.failed_count, 1);
        assert!(summary.has_failures());
        assert_eq!(summary.failures[0].feed_id, 3);
        assert_eq!(outcome.updated_count(), 1);
        assert_eq!(outcome.not_modified_count(), 1);
        assert_eq!(
            outcome.joined_failure_lines(),
            Some("https://example.com/three.xml: boom".to_string())
        );
    }

    #[tokio::test]
    async fn refresh_feed_commits_updated_entries_on_happy_path() {
        let service = RefreshService::new(
            Arc::new(SourceStub {
                outputs: Mutex::new(vec![FeedRefreshSourceOutput::Updated(FeedRefreshUpdate {
                    metadata: RefreshHttpMetadata {
                        etag: Some("new-etag".to_string()),
                        last_modified: Some("new-last-modified".to_string()),
                    },
                    feed: ParsedFeedUpdate {
                        title: Some("Example".to_string()),
                        site_url: Some(Url::parse("https://example.com").expect("valid url")),
                        description: Some("desc".to_string()),
                        entries: vec![sample_entry()],
                    },
                })]),
            }),
            Arc::new(StoreStub {
                targets: vec![sample_target(1, "https://example.com/feed.xml")],
                commits: Mutex::new(Vec::new()),
            }),
        );

        let outcome = service.refresh_feed(1).await.expect("refresh feed");

        assert!(matches!(outcome.result, super::RefreshFeedResult::Updated { entry_count: 1, .. }));
    }

    #[tokio::test]
    async fn refresh_feed_outcome_only_exposes_localization_candidates() {
        let service = RefreshService::new(
            Arc::new(SourceStub {
                outputs: Mutex::new(vec![FeedRefreshSourceOutput::Updated(FeedRefreshUpdate {
                    metadata: RefreshHttpMetadata::default(),
                    feed: ParsedFeedUpdate {
                        title: Some("Example".to_string()),
                        site_url: None,
                        description: None,
                        entries: vec![sample_entry(), sample_entry_without_html()],
                    },
                })]),
            }),
            Arc::new(StoreStub {
                targets: vec![sample_target(4, "https://example.com/feed.xml")],
                commits: Mutex::new(Vec::new()),
            }),
        );

        let outcome = service.refresh_feed(4).await.expect("refresh feed");

        match outcome.result {
            super::RefreshFeedResult::Updated { entry_count, localization_entries } => {
                assert_eq!(entry_count, 2);
                assert_eq!(localization_entries.len(), 1);
                assert_eq!(localization_entries[0].dedup_key, "entry-1");
                assert_eq!(localization_entries[0].title, "Entry");
                assert_eq!(localization_entries[0].content_html, "<p>summary</p>");
            }
            _ => panic!("expected updated outcome"),
        }
    }

    #[tokio::test]
    async fn refresh_feed_commits_failures_without_hiding_them() {
        let store = Arc::new(StoreStub {
            targets: vec![sample_target(2, "https://example.com/feed.xml")],
            commits: Mutex::new(Vec::new()),
        });
        let service = RefreshService::new(
            Arc::new(SourceStub {
                outputs: Mutex::new(vec![FeedRefreshSourceOutput::Failed(RefreshFailure {
                    message: "抓取订阅失败".to_string(),
                    metadata: None,
                })]),
            }),
            store.clone(),
        );

        let outcome = service.refresh_feed(2).await.expect("refresh feed");

        assert_eq!(outcome.failure_message(), Some("抓取订阅失败"));
        assert!(matches!(
            store.commits.lock().expect("lock commits")[0].1,
            RefreshCommit::Failed { .. }
        ));
    }

    #[tokio::test]
    async fn refresh_feed_handles_not_modified_without_rewriting_entries() {
        let store = Arc::new(StoreStub {
            targets: vec![sample_target(3, "https://example.com/feed.xml")],
            commits: Mutex::new(Vec::new()),
        });
        let service = RefreshService::new(
            Arc::new(SourceStub {
                outputs: Mutex::new(vec![FeedRefreshSourceOutput::NotModified(
                    RefreshHttpMetadata {
                        etag: Some("etag-2".to_string()),
                        last_modified: Some("lm-2".to_string()),
                    },
                )]),
            }),
            store.clone(),
        );

        let outcome = service.refresh_feed(3).await.expect("refresh feed");

        assert!(matches!(outcome.result, super::RefreshFeedResult::NotModified));
        assert!(matches!(
            store.commits.lock().expect("lock commits")[0].1,
            RefreshCommit::NotModified { .. }
        ));
    }

    #[tokio::test]
    async fn refresh_all_aggregates_per_feed_outcomes() {
        let service = RefreshService::new(
            Arc::new(SourceStub {
                outputs: Mutex::new(vec![
                    FeedRefreshSourceOutput::NotModified(RefreshHttpMetadata::default()),
                    FeedRefreshSourceOutput::Failed(RefreshFailure {
                        message: "boom".to_string(),
                        metadata: None,
                    }),
                ]),
            }),
            Arc::new(StoreStub {
                targets: vec![
                    sample_target(1, "https://example.com/one.xml"),
                    sample_target(2, "https://example.com/two.xml"),
                ],
                commits: Mutex::new(Vec::new()),
            }),
        );

        let outcome =
            service.refresh_all(RefreshAllInput { max_concurrency: 1 }).await.expect("refresh all");

        assert_eq!(outcome.not_modified_count(), 1);
        assert_eq!(outcome.failures().len(), 1);
    }
}
