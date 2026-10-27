use std::sync::Arc;

use anyhow::Context;
use rssr_domain::{
    ArchiveFilter, EntryIndexRepository, EntryQuery, EntrySummary, validate_entry_query,
    validate_mark_read_preview,
};

use crate::library_queries::LibraryQueryPort;

/// Entries a list returns at most, newest first (Develata, 2026-10-16): a response stays around
/// a megabyte on every platform. More matches set [`EntriesListOutcome::truncated`]; the page
/// asks to narrow the filter.
pub const MAX_LISTED_ENTRIES: u32 = 2000;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EntriesListOutcome {
    pub entries: Vec<EntrySummary>,
    /// 因自动归档而被本次查询排除掉的条目数，用于提示「已归档 N 篇」。
    ///
    /// 由一次 COUNT 查询得到，而不是把已归档的文章也读回来再在页面上数一遍。
    pub archived_count: u64,
    /// More entries matched than were returned (at most [`MAX_LISTED_ENTRIES`], or the query's
    /// own limit).
    #[serde(default)]
    pub truncated: bool,
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
        validate_entry_query(query)?;
        self.entry_repository.preview_mark_read(query).await.context("预览批量已读失败")
    }
    pub async fn mark_read_if_unchanged(
        &self,
        preview: &rssr_domain::MarkReadPreview,
    ) -> anyhow::Result<rssr_domain::MarkReadOutcome> {
        validate_mark_read_preview(preview)?;
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
        validate_entry_query(query)?;
        let archived_query = match query.archive_filter {
            ArchiveFilter::ExcludeArchived { cutoff } => Some(EntryQuery {
                archive_filter: ArchiveFilter::OnlyArchived { cutoff },
                ..query.clone()
            }),
            ArchiveFilter::ExcludeManual => {
                Some(EntryQuery { archive_filter: ArchiveFilter::OnlyManual, ..query.clone() })
            }
            ArchiveFilter::All | ArchiveFilter::OnlyArchived { .. } | ArchiveFilter::OnlyManual => {
                None
            }
        };
        // One entry past the bound tells whether more matched, without a second query.
        let shown = query.limit.unwrap_or(MAX_LISTED_ENTRIES).min(MAX_LISTED_ENTRIES);
        let bounded = EntryQuery { limit: Some(shown + 1), ..query.clone() };
        let mut page = self
            .queries
            .entries_page(&bounded, archived_query.as_ref())
            .await
            .context("读取文章失败")?;
        let truncated = page.entries.len() > shown as usize;
        page.entries.truncate(shown as usize);

        Ok(EntriesListOutcome {
            entries: page.entries,
            archived_count: page.counted.unwrap_or(0),
            truncated,
        })
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

        async fn list_entries(&self, query: &EntryQuery) -> rssr_domain::Result<Vec<EntrySummary>> {
            let limit = query.limit.map_or(usize::MAX, |limit| limit as usize);
            Ok(self.entries.iter().take(limit).cloned().collect())
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

    fn entries(count: i64) -> Vec<EntrySummary> {
        (0..count)
            .map(|id| EntrySummary {
                id,
                feed_id: 1,
                title: format!("Entry {id}"),
                feed_title: "Feed".to_string(),
                published_at: None,
                is_read: false,
                is_starred: false,
            })
            .collect()
    }

    /// A list returns at most `MAX_LISTED_ENTRIES` (or the query's smaller limit) and says when
    /// more matched; exactly the bound is not truncated.
    #[tokio::test]
    async fn a_list_stops_at_its_bound_and_says_so() {
        let max = super::MAX_LISTED_ENTRIES as i64;
        for (available, limit, shown, truncated) in [
            (max + 5, None, max, true),
            (max, None, max, false),
            (max - 1, None, max - 1, false),
            (max + 5, Some(10), 10, true),
            (10, Some(10), 10, false),
            (max + 5, Some(u32::MAX), max, true),
        ] {
            let repository =
                Arc::new(EntryRepositoryStub { entries: entries(available), ..Default::default() });
            let outcome = service(repository)
                .list_entries(&EntryQuery { limit, ..EntryQuery::default() })
                .await
                .unwrap();
            assert_eq!(
                (outcome.entries.len() as i64, outcome.truncated),
                (shown, truncated),
                "{available} available, limit {limit:?}"
            );
        }
    }

    /// A confirmation carrying more ids than any preview holds is refused before the store.
    #[tokio::test]
    async fn a_confirmation_over_the_bound_never_reaches_the_store() {
        let service = service(Arc::new(EntryRepositoryStub::default()));
        let ids = (0..=rssr_domain::MAX_MARK_READ_SELECTION as i64).collect();
        let forged =
            rssr_domain::MarkReadPreview { query: EntryQuery::default(), unread_entry_ids: ids };
        let error = service.mark_read_if_unchanged(&forged).await.unwrap_err();
        let Some(rssr_domain::DomainError::InvalidInput(message)) =
            error.downcast_ref::<rssr_domain::DomainError>()
        else {
            panic!("{error:#}");
        };
        assert!(message.starts_with("范围过大"), "the stub was reached: {message}");

        let at_bound = rssr_domain::MarkReadPreview {
            unread_entry_ids: forged.unread_entry_ids[1..].to_vec(),
            ..forged
        };
        let error = service.mark_read_if_unchanged(&at_bound).await.unwrap_err();
        assert!(format!("{error:#}").contains("此测试替身"), "{error:#}");
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
