use std::sync::Arc;

use anyhow::Context;
use rssr_domain::{DomainError, EntryIndexRepository};

/// What setting an entry's read or starred flag to a value did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryFlagOutcome {
    /// The entry now has `value`; `changed` is false when it already had it (nothing written).
    Set { value: bool, changed: bool },
    /// The entry no longer exists, removed by a refresh or an unsubscribe since it was shown.
    Missing,
}

/// Sets entry flags to target values, for the entry list and the reader alike. Repeating a
/// request changes nothing; an older retry can still undo a newer opposite request (no versions,
/// `docs/design/v0.2-phase-b2-contracts.md`, D3).
#[derive(Clone)]
pub struct EntryFlagsService {
    entry_repository: Arc<dyn EntryIndexRepository>,
}

impl EntryFlagsService {
    pub fn new(entry_repository: Arc<dyn EntryIndexRepository>) -> Self {
        Self { entry_repository }
    }

    pub async fn set_read(&self, entry_id: i64, read: bool) -> anyhow::Result<EntryFlagOutcome> {
        outcome(read, self.entry_repository.set_read(entry_id, read).await)
            .context("更新已读状态失败")
    }

    pub async fn set_starred(
        &self,
        entry_id: i64,
        starred: bool,
    ) -> anyhow::Result<EntryFlagOutcome> {
        outcome(starred, self.entry_repository.set_starred(entry_id, starred).await)
            .context("更新收藏状态失败")
    }
}

fn outcome(value: bool, write: rssr_domain::Result<bool>) -> rssr_domain::Result<EntryFlagOutcome> {
    match write {
        Ok(changed) => Ok(EntryFlagOutcome::Set { value, changed }),
        Err(DomainError::NotFound) => Ok(EntryFlagOutcome::Missing),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use rssr_domain::{
        DomainError, EntryIndexRepository, EntryNavigation, EntryQuery, EntryRecord, EntrySummary,
    };

    use super::{EntryFlagOutcome, EntryFlagsService};

    /// One entry (id 1) whose flags start cleared; records every write it is asked for.
    #[derive(Default)]
    struct OneEntry {
        flags: Mutex<(bool, bool)>,
        writes: Mutex<Vec<(&'static str, i64, bool)>>,
        fail: bool,
    }

    impl OneEntry {
        fn set(
            &self,
            which: &'static str,
            entry_id: i64,
            value: bool,
        ) -> rssr_domain::Result<bool> {
            self.writes.lock().unwrap().push((which, entry_id, value));
            if self.fail {
                return Err(DomainError::Persistence("disk full".into()));
            }
            if entry_id != 1 {
                return Err(DomainError::NotFound);
            }
            let mut flags = self.flags.lock().unwrap();
            let flag = if which == "read" { &mut flags.0 } else { &mut flags.1 };
            let changed = *flag != value;
            *flag = value;
            Ok(changed)
        }
    }

    #[async_trait::async_trait]
    impl EntryIndexRepository for OneEntry {
        async fn preview_mark_read(
            &self,
            _query: &EntryQuery,
        ) -> rssr_domain::Result<rssr_domain::MarkReadPreview> {
            unreachable!()
        }
        async fn mark_read_if_unchanged(
            &self,
            _preview: &rssr_domain::MarkReadPreview,
        ) -> rssr_domain::Result<rssr_domain::MarkReadOutcome> {
            unreachable!()
        }
        async fn list_entries(
            &self,
            _query: &EntryQuery,
        ) -> rssr_domain::Result<Vec<EntrySummary>> {
            unreachable!()
        }
        async fn count_entries(&self, _query: &EntryQuery) -> rssr_domain::Result<u64> {
            unreachable!()
        }
        async fn get_entry_record(&self, _id: i64) -> rssr_domain::Result<Option<EntryRecord>> {
            unreachable!()
        }
        async fn reader_navigation(&self, _id: i64) -> rssr_domain::Result<EntryNavigation> {
            unreachable!()
        }
        async fn set_read(&self, entry_id: i64, is_read: bool) -> rssr_domain::Result<bool> {
            self.set("read", entry_id, is_read)
        }
        async fn set_starred(&self, entry_id: i64, is_starred: bool) -> rssr_domain::Result<bool> {
            self.set("starred", entry_id, is_starred)
        }
        async fn delete_for_feed(&self, _feed_id: i64) -> rssr_domain::Result<()> {
            unreachable!()
        }
    }

    #[tokio::test]
    async fn the_target_value_is_written_and_a_repeat_changes_nothing() {
        let entry = Arc::new(OneEntry::default());
        let service = EntryFlagsService::new(entry.clone());

        assert_eq!(
            service.set_read(1, true).await.unwrap(),
            EntryFlagOutcome::Set { value: true, changed: true }
        );
        assert_eq!(
            service.set_read(1, true).await.unwrap(),
            EntryFlagOutcome::Set { value: true, changed: false }
        );
        assert_eq!(
            service.set_starred(1, false).await.unwrap(),
            EntryFlagOutcome::Set { value: false, changed: false }
        );
        assert_eq!(
            *entry.writes.lock().unwrap(),
            [("read", 1, true), ("read", 1, true), ("starred", 1, false)]
        );
    }

    #[tokio::test]
    async fn a_missing_entry_is_an_outcome_and_other_failures_are_errors() {
        let service = EntryFlagsService::new(Arc::new(OneEntry::default()));
        assert_eq!(service.set_starred(2, true).await.unwrap(), EntryFlagOutcome::Missing);

        let failing =
            EntryFlagsService::new(Arc::new(OneEntry { fail: true, ..Default::default() }));
        assert!(failing.set_read(1, true).await.is_err());
    }
}
