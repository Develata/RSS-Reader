use std::sync::Arc;

use anyhow::Context;
use rssr_domain::{ArchiveFilter, EntryIndexRepository, EntryQuery, EntrySummary};

use crate::library_queries::LibraryQueryPort;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntriesListOutcome {
    pub entries: Vec<EntrySummary>,
    /// 因自动归档而被本次查询排除掉的条目数，用于提示「已归档 N 篇」。
    ///
    /// 由一次 COUNT 查询得到，而不是把已归档的文章也读回来再在页面上数一遍。
    pub archived_count: u64,
}

#[derive(Clone)]
pub struct EntriesListService {
    entry_repository: Arc<dyn EntryIndexRepository>,
    queries: Arc<dyn LibraryQueryPort>,
}

impl EntriesListService {
    pub async fn preview_mark_read(
        &self,
        query: &EntryQuery,
    ) -> anyhow::Result<rssr_domain::MarkReadPreview> {
        self.entry_repository.preview_mark_read(query).await.context("预览批量已读失败")
    }
    pub async fn mark_read_if_unchanged(
        &self,
        preview: &rssr_domain::MarkReadPreview,
    ) -> anyhow::Result<rssr_domain::MarkReadOutcome> {
        self.entry_repository.mark_read_if_unchanged(preview).await.context("批量标为已读失败")
    }

    pub fn new(
        entry_repository: Arc<dyn EntryIndexRepository>,
        queries: Arc<dyn LibraryQueryPort>,
    ) -> Self {
        Self { entry_repository, queries }
    }

    /// 读取一页文章，并在同一个读快照里数出被归档隐藏的条目数。
    ///
    /// 归档筛选与计数都发生在存储层：页面层不再为了「数一下有多少被归档」而把已归档的文章
    /// 全量读回内存；同一快照保证计数与这一页对应同一份数据。
    pub async fn list_entries(&self, query: &EntryQuery) -> anyhow::Result<EntriesListOutcome> {
        let archived_query = match query.archive_filter {
            ArchiveFilter::ExcludeArchived { cutoff } => Some(EntryQuery {
                archive_filter: ArchiveFilter::OnlyArchived { cutoff },
                ..query.clone()
            }),
            ArchiveFilter::All | ArchiveFilter::OnlyArchived { .. } => None,
        };
        let page = self
            .queries
            .entries_page(query, archived_query.as_ref())
            .await
            .context("读取文章失败")?;

        Ok(EntriesListOutcome { entries: page.entries, archived_count: page.counted.unwrap_or(0) })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use rssr_domain::{
        ArchiveFilter, EntryIndexRepository, EntryNavigation, EntryQuery, EntryRecord, EntrySummary,
    };
    use time::OffsetDateTime;

    use super::EntriesListService;
    use crate::library_queries::test_support::RepositoryQueries;

    #[derive(Debug, Default)]
    struct EntryRepositoryStub {
        entries: Vec<EntrySummary>,
        counted: Mutex<Vec<EntryQuery>>,
    }

    #[async_trait::async_trait]
    impl EntryIndexRepository for EntryRepositoryStub {
        async fn preview_mark_read(
            &self,
            _query: &EntryQuery,
        ) -> rssr_domain::Result<rssr_domain::MarkReadPreview> {
            Err(rssr_domain::DomainError::InvalidInput("此测试替身不支持批量操作".into()))
        }
        async fn mark_read_if_unchanged(
            &self,
            _preview: &rssr_domain::MarkReadPreview,
        ) -> rssr_domain::Result<rssr_domain::MarkReadOutcome> {
            Err(rssr_domain::DomainError::InvalidInput("此测试替身不支持批量操作".into()))
        }

        async fn list_entries(
            &self,
            _query: &EntryQuery,
        ) -> rssr_domain::Result<Vec<EntrySummary>> {
            Ok(self.entries.clone())
        }

        async fn count_entries(&self, query: &EntryQuery) -> rssr_domain::Result<u64> {
            self.counted.lock().expect("lock counted").push(query.clone());
            Ok(self.entries.len() as u64)
        }

        async fn get_entry_record(
            &self,
            _entry_id: i64,
        ) -> rssr_domain::Result<Option<EntryRecord>> {
            Ok(None)
        }

        async fn reader_navigation(
            &self,
            _current_entry_id: i64,
        ) -> rssr_domain::Result<EntryNavigation> {
            Ok(EntryNavigation::default())
        }

        async fn set_read(&self, _entry_id: i64, _is_read: bool) -> rssr_domain::Result<bool> {
            unreachable!("the entry list does not set flags")
        }

        async fn set_starred(&self, _entry_id: i64, _starred: bool) -> rssr_domain::Result<bool> {
            unreachable!("the entry list does not set flags")
        }

        async fn delete_for_feed(&self, _feed_id: i64) -> rssr_domain::Result<()> {
            Ok(())
        }
    }

    fn service(repository: Arc<EntryRepositoryStub>) -> EntriesListService {
        let queries =
            RepositoryQueries { entries: Some(repository.clone()), ..RepositoryQueries::default() };
        EntriesListService::new(repository, Arc::new(queries))
    }

    #[tokio::test]
    async fn list_entries_returns_entry_summaries() {
        let repository = Arc::new(EntryRepositoryStub {
            entries: vec![EntrySummary {
                id: 42,
                feed_id: 7,
                title: "Entry 42".to_string(),
                feed_title: "Feed".to_string(),
                published_at: None,
                is_read: false,
                is_starred: false,
            }],
            ..EntryRepositoryStub::default()
        });

        let outcome = service(repository.clone())
            .list_entries(&EntryQuery::default())
            .await
            .expect("list entries");

        assert_eq!(outcome.entries.len(), 1);
        assert_eq!(outcome.entries[0].id, 42);
        assert_eq!(outcome.archived_count, 0);
        assert!(repository.counted.lock().expect("lock counted").is_empty());
    }

    /// Excluding archived entries asks for the archived count with the same query otherwise.
    #[tokio::test]
    async fn excluding_archived_entries_counts_them_with_the_same_query() {
        let repository = Arc::new(EntryRepositoryStub::default());
        let cutoff = OffsetDateTime::UNIX_EPOCH;
        let query = EntryQuery {
            feed_id: Some(7),
            archive_filter: ArchiveFilter::ExcludeArchived { cutoff },
            ..EntryQuery::default()
        };

        let outcome = service(repository.clone()).list_entries(&query).await.expect("list");

        assert_eq!(outcome.archived_count, 0);
        let counted = repository.counted.lock().expect("lock counted");
        assert_eq!(
            *counted,
            [EntryQuery { archive_filter: ArchiveFilter::OnlyArchived { cutoff }, ..query }]
        );
    }
}
