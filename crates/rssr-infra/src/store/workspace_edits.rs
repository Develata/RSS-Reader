//! Reading-workspace writes over the same SQL library on native hosts and Pages.
//! Each operation owns one transaction; no network work runs under the write lock.
use anyhow::Context;
use rssr_application::{EntryBatchPort, FeedEditPort};
use rssr_domain::{
    AppStateSnapshot, DomainError, EntryBatchAction, EntryBatchRequest, FeedEditDetails,
    FeedEditRequest,
};
use rusqlite::{OptionalExtension, params};

use super::{
    LibraryStore, StoreError,
    codec::now,
    documents::{self, APP_STATE_KEY},
};

#[derive(Clone)]
pub struct LibraryWorkspaceEdits {
    store: LibraryStore,
}

impl LibraryWorkspaceEdits {
    pub fn new(store: LibraryStore) -> Self {
        Self { store }
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl EntryBatchPort for LibraryWorkspaceEdits {
    async fn apply_entry_batch(&self, request: EntryBatchRequest) -> anyhow::Result<u64> {
        let ids = serde_json::to_string(&request.entry_ids)?;
        self.store.write(move |tx| {
            let update = match request.action {
                EntryBatchAction::MarkRead => "UPDATE entries SET is_read = 1, read_at = ?1, updated_at = ?1 WHERE is_read = 0",
                EntryBatchAction::Archive => "UPDATE entries SET is_archived = 1, updated_at = ?1 WHERE is_archived = 0",
            };
            let sql = format!("{update} AND id IN (SELECT value FROM json_each(?2)) AND feed_id IN (SELECT id FROM feeds WHERE is_deleted = 0)");
            Ok(tx.prepare_cached(&sql)?.execute(params![now(), ids])? as u64)
        }).await.map_err(DomainError::from).context("更新所选文章失败")
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl FeedEditPort for LibraryWorkspaceEdits {
    async fn edit_details(&self, feed_id: i64) -> anyhow::Result<FeedEditDetails> {
        self.store
            .read(move |conn| {
                conn.prepare_cached(
                    "SELECT url, title, custom_title FROM feeds WHERE id = ?1 AND is_deleted = 0",
                )?
                .query_row([feed_id], |row| {
                    Ok(FeedEditDetails {
                        feed_id,
                        url: row.get(0)?,
                        original_title: row.get(1)?,
                        custom_title: row.get(2)?,
                    })
                })
                .optional()?
                .ok_or(StoreError::NotFound)
            })
            .await
            .map_err(DomainError::from)
            .context("读取来源失败")
    }

    async fn update_subscription(&self, edit: FeedEditRequest) -> anyhow::Result<()> {
        self.store
            .write(move |tx| {
                let old_url: String = tx
                    .prepare_cached("SELECT url FROM feeds WHERE id = ?1 AND is_deleted = 0")?
                    .query_row([edit.feed_id], |row| row.get(0))
                    .optional()?
                    .ok_or(StoreError::NotFound)?;
                let duplicate: bool = tx
                    .prepare_cached(
                        "SELECT EXISTS(SELECT 1 FROM feeds WHERE url = ?1 AND id != ?2)",
                    )?
                    .query_row(params![edit.url, edit.feed_id], |row| row.get(0))?;
                if duplicate {
                    return Err(DomainError::Conflict(
                        "该地址已属于另一个订阅，请使用不同的地址".into(),
                    )
                    .into());
                }
                let changed_url = old_url != edit.url;
                let timestamp = now();
                tx.prepare_cached(
                    "UPDATE feeds SET url = ?2, custom_title = ?3, updated_at = ?4,
                generation = generation + ?5,
                etag = CASE WHEN ?5 THEN NULL ELSE etag END,
                last_modified = CASE WHEN ?5 THEN NULL ELSE last_modified END,
                last_fetched_at = CASE WHEN ?5 THEN NULL ELSE last_fetched_at END,
                last_success_at = CASE WHEN ?5 THEN NULL ELSE last_success_at END,
                fetch_error = CASE WHEN ?5 THEN NULL ELSE fetch_error END,
                fetch_error_kind = CASE WHEN ?5 THEN NULL ELSE fetch_error_kind END,
                site_url = CASE WHEN ?5 THEN NULL ELSE site_url END,
                icon_url = CASE WHEN ?5 THEN NULL ELSE icon_url END,
                description = CASE WHEN ?5 THEN NULL ELSE description END WHERE id = ?1",
                )?
                .execute(params![
                    edit.feed_id,
                    edit.url,
                    edit.custom_title,
                    timestamp,
                    changed_url
                ])?;
                if changed_url {
                    let mut state: AppStateSnapshot = documents::load(tx, APP_STATE_KEY)?;
                    let mut changed = false;
                    for url in &mut state.entries_workspace.selected_feed_urls {
                        if *url == old_url {
                            *url = edit.url.clone();
                            changed = true;
                        }
                    }
                    if changed {
                        state.entries_workspace.selected_feed_urls.sort();
                        state.entries_workspace.selected_feed_urls.dedup();
                        documents::save(tx, APP_STATE_KEY, &state, &timestamp)?;
                    }
                }
                Ok(())
            })
            .await
            .map_err(DomainError::from)
            .context("保存来源失败")
    }
}
