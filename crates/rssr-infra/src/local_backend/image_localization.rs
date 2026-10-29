//! Native image localization: bounded background rounds and reader-triggered work.
//! Queue policy lives in `background`; SQL identity checks stay in the store.
mod background;
use crate::fetch::BodyAssetLocalizer;
use crate::store::{LibraryEntries, LocalizedBody};
use anyhow::Context;
use background::{BackgroundQueue, PendingFeed};
use rssr_application::RefreshLocalizationCandidate;
use std::sync::Arc;
use std::time::Duration;
#[derive(Clone)]
pub(super) struct ImageLocalizationWorker {
    entry_repository: Arc<LibraryEntries>,
    background_asset_localizer: BodyAssetLocalizer,
    reader_asset_localizer: BodyAssetLocalizer,
    background: Arc<BackgroundQueue>,
}

impl ImageLocalizationWorker {
    /// 每次刷新为一个订阅最多尝试本地化的条目数，计数规则见 [`localize_feed_entries`]（审计 C2）。
    const MAX_BACKGROUND_ATTEMPTS: usize = 5;
    /// 一个订阅一轮后台本地化的总时长上限。
    const BACKGROUND_BUDGET: Duration = Duration::from_secs(180);
    /// 同时进行后台本地化的订阅数。
    const MAX_CONCURRENT_FEEDS: usize = 2;

    pub(super) fn new(
        entry_repository: Arc<LibraryEntries>,
        background_asset_localizer: BodyAssetLocalizer,
        reader_asset_localizer: BodyAssetLocalizer,
    ) -> Self {
        Self {
            entry_repository,
            background_asset_localizer,
            reader_asset_localizer,
            background: Arc::new(BackgroundQueue::new(Self::MAX_CONCURRENT_FEEDS)),
        }
    }

    pub(super) fn spawn(
        &self,
        feed_id: i64,
        entries: Vec<RefreshLocalizationCandidate>,
    ) -> Option<tokio::task::JoinHandle<()>> {
        // Admission bounds metadata before spawning. Queued jobs retain no HTML;
        // load one body only after obtaining one of the two execution slots.
        let (pending, entries) = self.background.enqueue(feed_id, entries)?;
        let entry_repository = self.entry_repository.clone();
        let localizer = self.background_asset_localizer.clone();
        let background = self.background.clone();

        Some(tokio::spawn(async move {
            let pending = pending;
            let Ok(_slot) = background.slots.acquire().await else {
                return;
            };
            let round = localize_feed_entries(
                &entry_repository,
                &localizer,
                feed_id,
                entries,
                Self::MAX_BACKGROUND_ATTEMPTS,
                &pending,
            );
            if tokio::time::timeout(Self::BACKGROUND_BUDGET, round).await.is_err() {
                tracing::warn!(
                    feed_id,
                    budget_secs = Self::BACKGROUND_BUDGET.as_secs(),
                    "后台正文图片本地化达到本轮时长上限，剩余文章留到下次刷新"
                );
            }
        }))
    }

    pub(super) async fn localize_entry_on_demand(&self, entry_id: i64) -> anyhow::Result<bool> {
        // Capture the feed generation before starting network work. If deletion/re-add happens
        // while images are being localized, the final write is rejected as stale.
        let Some((feed_id, generation)) = self
            .entry_repository
            .active_entry_generation(entry_id)
            .await
            .context("读取当前文章所属订阅 generation 失败")?
        else {
            return Ok(false);
        };
        let Some(entry) =
            self.entry_repository.get_entry(entry_id).await.context("读取当前文章失败")?
        else {
            return Ok(false);
        };
        if entry.feed_id != feed_id {
            return Ok(false);
        }

        let Some(original_html) = entry.content_html.as_deref().map(Arc::<str>::from) else {
            return Ok(false);
        };

        // 写回比较的是读到的存储哈希本身（可能是旧版本留下的 NULL），不能用重新计算的值代替，
        // 否则 NULL 行永远写不回（独立评审）。
        let expected_content_hash = entry.content_hash.clone();

        let localizer = self.reader_asset_localizer.clone();
        let localized_html = match tokio::time::timeout(
            localization_timeout(&localizer),
            localizer.localize_html_images_reporting(&original_html, entry.url.as_ref()),
        )
        .await
        {
            // 一张图都没换成本地副本时不写回：规范化阅读页显示时会做，写回只会让这篇文章被
            // 当成“已本地化”而不再重试（审计 C1 复审）。
            Ok(outcome) if outcome.localized_images > 0 => outcome.html,
            Ok(_) => return Ok(false),
            Err(_) => {
                tracing::warn!(
                    entry_id,
                    feed_id = entry.feed_id,
                    entry_url = ?entry.url.as_ref().map(crate::outbound::redacted),
                    reason = "timeout",
                    timeout_secs = localization_timeout(&localizer).as_secs(),
                    "当前阅读文章正文图片按需本地化超时，保留原始 HTML"
                );
                return Ok(false);
            }
        };

        let updated = self
            .entry_repository
            .update_localized_body(
                feed_id,
                generation,
                LocalizedBody {
                    dedup_key: entry.dedup_key,
                    expected_content_hash,
                    base_html: original_html,
                    localized_html,
                },
            )
            .await
            .context("写回按需本地化后的正文失败")?;

        Ok(updated)
    }
}

/// 依次尝试本地化一个订阅的文章，返回计入预算的尝试篇数（审计 C2）。
///
/// 为一篇文章发起过图片请求就计一次尝试，不论成败或超时，达到 `max_attempts` 即停。不计数的：
/// 正文已是这份源正文的派生版本（源未变的刷新会保留它，见审计 C1）而跳过的，以及无图、正文超出
/// 预算等根本没有发请求的。每检查一篇便推进轮转位置，失败和超时也不会阻塞后续文章。
async fn localize_feed_entries(
    entry_repository: &LibraryEntries,
    localizer: &BodyAssetLocalizer,
    feed_id: i64,
    entries: Vec<RefreshLocalizationCandidate>,
    max_attempts: usize,
    pending: &PendingFeed,
) -> usize {
    let mut attempts = 0_usize;
    for entry in entries {
        if attempts >= max_attempts {
            break;
        }
        pending.examined(&entry);
        let source = match entry_repository
            .body_for_localization(
                feed_id,
                entry.generation,
                entry.dedup_key.clone(),
                localizer.max_html_bytes(),
            )
            .await
        {
            Ok(Some(source))
                if entry.matches_stored_body(&source.html, source.content_hash.as_deref()) =>
            {
                source
            }
            Ok(_) => continue,
            Err(error) => {
                tracing::warn!(
                    feed_id,
                    dedup_key = %entry.dedup_key,
                    error = %error,
                    "读取已存正文失败，跳过后台图片本地化"
                );
                continue;
            }
        };
        let original_html: Arc<str> = Arc::from(source.html);

        let localized_html = match tokio::time::timeout(
            localization_timeout(localizer),
            localizer.localize_html_images_reporting(&original_html, entry.url.as_ref()),
        )
        .await
        {
            Ok(outcome) => {
                if outcome.requested_images {
                    attempts += 1;
                }
                // 没有图片换成本地副本就不写回，见 `localize_entry_on_demand`。
                if outcome.localized_images == 0 {
                    continue;
                }
                outcome.html
            }
            Err(_) => {
                // 超时只可能发生在发出图片请求之后。
                attempts += 1;
                tracing::warn!(
                    feed_id,
                    entry_url = ?entry.url.as_ref().map(crate::outbound::redacted),
                    reason = "timeout",
                    timeout_secs = localization_timeout(localizer).as_secs(),
                    "后台正文图片本地化超时，跳过当前文章"
                );
                continue;
            }
        };

        let update = LocalizedBody {
            dedup_key: entry.dedup_key.clone(),
            expected_content_hash: source.content_hash,
            base_html: original_html,
            localized_html,
        };
        match entry_repository.update_localized_body(feed_id, entry.generation, update).await {
            Ok(true) => {}
            Ok(false) => {
                tracing::debug!(
                    feed_id,
                    dedup_key = %entry.dedup_key,
                    "跳过后台正文图片本地化写回：文章内容已被更新"
                );
            }
            Err(error) => {
                tracing::warn!(
                    feed_id,
                    dedup_key = %entry.dedup_key,
                    error = %error,
                    "写回后台本地化后的正文失败"
                );
            }
        }
    }
    attempts
}

fn localization_timeout(localizer: &BodyAssetLocalizer) -> Duration {
    Duration::from_secs(
        localizer.image_request_timeout().as_secs() * localizer.max_images_per_entry() as u64 + 5,
    )
}

#[cfg(test)]
mod tests;
