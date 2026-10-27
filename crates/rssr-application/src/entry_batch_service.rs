//! Commands act on the IDs selected by the user, never on a later, broader query.
use std::sync::Arc;

use anyhow::Context;
use rssr_domain::{DomainError, EntryBatchRequest, MAX_ENTRIES_PAGE_SIZE};

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
pub trait EntryBatchPort: Send + Sync {
    /// One atomic write of the validated IDs; missing/deleted entries are ignored.
    /// Returns only the number whose flag changed. Repeating the command is idempotent.
    async fn apply_entry_batch(&self, request: EntryBatchRequest) -> anyhow::Result<u64>;
}

#[derive(Clone)]
pub struct EntryBatchService {
    port: Arc<dyn EntryBatchPort>,
}

impl EntryBatchService {
    pub fn new(port: Arc<dyn EntryBatchPort>) -> Self {
        Self { port }
    }

    pub async fn apply(&self, mut request: EntryBatchRequest) -> anyhow::Result<u64> {
        if request.entry_ids.is_empty()
            || request.entry_ids.len() > MAX_ENTRIES_PAGE_SIZE as usize
            || request.entry_ids.iter().any(|id| *id <= 0)
        {
            return Err(DomainError::InvalidInput("请选择单页范围内的有效文章".into()).into());
        }
        request.entry_ids.sort_unstable();
        request.entry_ids.dedup();
        self.port.apply_entry_batch(request).await.context("处理所选文章失败")
    }
}
