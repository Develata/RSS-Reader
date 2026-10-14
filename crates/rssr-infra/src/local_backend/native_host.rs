//! The host of the desktop app, Android and (later) the server: tokio, WebDAV, and image
//! localization into the library.

use std::sync::{Arc, PoisonError};
use std::time::Duration;

use anyhow::Context;
use rssr_application::{
    DEFAULT_REFRESH_CONCURRENCY, RefreshLocalizedEntry, RemoteConfigStore,
    SERIAL_REFRESH_CONCURRENCY,
};
use time::OffsetDateTime;

use super::{LocalBackend, LocalHost, LocalTask};
use crate::composition::compose_library_use_cases;
use crate::config_sync::webdav::WebDavConfigSync;
use crate::fetch::BodyAssetLocalizer;
use crate::store::{LibraryEntries, LibraryStore, LocalizedBody, compute_entry_content_hash};

/// The backend over an opened library, with this host.
pub fn native_local_backend(store: LibraryStore) -> LocalBackend<NativeHost> {
    // 实测的 journal 模式决定并发度：WAL 没启用成功时退回串行，
    // 否则并发写者会互相干等并把前台查询一起卡住。
    //
    // 不能假定 WAL 启用成功：`PRAGMA journal_mode=WAL` 在不支持共享内存的文件系统上
    // 会静默留在回滚日志模式，而那种模式下并发写者只能互相干等、还会阻塞读。
    let journal_mode = store.journal_mode().to_owned();
    let refresh_concurrency = if journal_mode == "wal" {
        DEFAULT_REFRESH_CONCURRENCY
    } else {
        tracing::warn!(
            journal_mode = %journal_mode,
            "数据库未运行在 WAL 模式（可能位于不支持共享内存的文件系统），刷新退回串行以避免写者互相阻塞"
        );
        SERIAL_REFRESH_CONCURRENCY
    };
    tracing::info!(journal_mode = %journal_mode, refresh_concurrency, "本地数据库就绪");

    let composition = compose_library_use_cases(store);
    LocalBackend::new(
        composition.use_cases,
        NativeHost {
            image_localization_worker: ImageLocalizationWorker::new(composition.entries),
            refresh_concurrency,
        },
    )
}

/// tokio for background work and time, WebDAV for configuration exchange, and image
/// localization into the library.
pub struct NativeHost {
    image_localization_worker: ImageLocalizationWorker,
    refresh_concurrency: usize,
}

#[async_trait::async_trait]
impl LocalHost for NativeHost {
    fn refresh_concurrency(&self) -> usize {
        self.refresh_concurrency
    }

    fn now_utc(&self) -> OffsetDateTime {
        OffsetDateTime::now_utc()
    }

    fn spawn(&self, task: LocalTask) {
        tokio::spawn(task);
    }

    async fn sleep(&self, duration: Duration) {
        tokio::time::sleep(duration).await;
    }

    fn remote_config_store(
        &self,
        endpoint: &str,
        remote_path: &str,
    ) -> anyhow::Result<Box<dyn RemoteConfigStore>> {
        // An endpoint that cannot be used is the user's to correct.
        let store = WebDavConfigSync::new(endpoint, remote_path)
            .map_err(|error| rssr_domain::DomainError::InvalidInput(format!("{error:#}")))?;
        Ok(Box::new(store))
    }

    fn feed_refreshed(&self, feed_id: i64, entries: Vec<RefreshLocalizedEntry>) {
        self.image_localization_worker.spawn(feed_id, entries);
    }

    async fn localize_entry_assets(&self, entry_id: i64) -> anyhow::Result<bool> {
        self.image_localization_worker.localize_entry_on_demand(entry_id).await
    }
}

#[derive(Clone)]
struct ImageLocalizationWorker {
    entry_repository: Arc<LibraryEntries>,
    background_asset_localizer: BodyAssetLocalizer,
    reader_asset_localizer: BodyAssetLocalizer,
    background: Arc<BackgroundLocalizationQueue>,
}

/// 后台图片本地化的并发与排队约束（审计 C2）。
struct BackgroundLocalizationQueue {
    slots: tokio::sync::Semaphore,
    pending_feeds: std::sync::Mutex<std::collections::HashSet<i64>>,
}

/// 某个订阅已有排队或进行中的后台本地化；任务结束（包括被取消）时释放。
struct PendingFeed {
    queue: Arc<BackgroundLocalizationQueue>,
    feed_id: i64,
}

impl PendingFeed {
    fn claim(queue: &Arc<BackgroundLocalizationQueue>, feed_id: i64) -> Option<Self> {
        let mut pending = queue.pending_feeds.lock().unwrap_or_else(PoisonError::into_inner);
        pending.insert(feed_id).then(|| Self { queue: Arc::clone(queue), feed_id })
    }
}

impl Drop for PendingFeed {
    fn drop(&mut self) {
        let mut pending = self.queue.pending_feeds.lock().unwrap_or_else(PoisonError::into_inner);
        pending.remove(&self.feed_id);
    }
}

impl ImageLocalizationWorker {
    /// 每次刷新为一个订阅最多尝试本地化的条目数，计数规则见 [`localize_feed_entries`]（审计 C2）。
    const MAX_BACKGROUND_ATTEMPTS: usize = 5;
    /// 一个订阅一轮后台本地化的总时长上限。
    const BACKGROUND_BUDGET: Duration = Duration::from_secs(180);
    /// 同时进行后台本地化的订阅数。
    const MAX_CONCURRENT_FEEDS: usize = 2;

    fn new(entry_repository: Arc<LibraryEntries>) -> Self {
        Self {
            entry_repository,
            background_asset_localizer: BodyAssetLocalizer::new(),
            reader_asset_localizer: BodyAssetLocalizer::for_reader_entry(),
            background: Arc::new(BackgroundLocalizationQueue {
                slots: tokio::sync::Semaphore::new(Self::MAX_CONCURRENT_FEEDS),
                pending_feeds: Default::default(),
            }),
        }
    }

    fn spawn(&self, feed_id: i64, entries: Vec<RefreshLocalizedEntry>) {
        // 同一订阅已有排队或进行中的后台本地化时不再追加：多轮自动刷新不会叠加任务与正文副本，
        // 这次没轮到的条目下次刷新再处理。先到的列表即使过时也只会少做、不会写错：写回按源正文
        // 哈希与 generation 校验。
        let Some(pending) = PendingFeed::claim(&self.background, feed_id) else {
            return;
        };
        let entry_repository = self.entry_repository.clone();
        let localizer = self.background_asset_localizer.clone();
        let background = self.background.clone();

        tokio::spawn(async move {
            let _pending = pending;
            let Ok(_slot) = background.slots.acquire().await else {
                return;
            };
            let round = localize_feed_entries(
                &entry_repository,
                &localizer,
                feed_id,
                entries,
                Self::MAX_BACKGROUND_ATTEMPTS,
            );
            if tokio::time::timeout(Self::BACKGROUND_BUDGET, round).await.is_err() {
                tracing::warn!(
                    feed_id,
                    budget_secs = Self::BACKGROUND_BUDGET.as_secs(),
                    "后台正文图片本地化达到本轮时长上限，剩余文章留到下次刷新"
                );
            }
        });
    }

    async fn localize_entry_on_demand(&self, entry_id: i64) -> anyhow::Result<bool> {
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
                    entry_url = ?entry.url,
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
/// 预算等根本没有发请求的。图片持续失败的文章每轮仍会占用尝试次数，直到源正文变化。
async fn localize_feed_entries(
    entry_repository: &LibraryEntries,
    localizer: &BodyAssetLocalizer,
    feed_id: i64,
    entries: Vec<RefreshLocalizedEntry>,
    max_attempts: usize,
) -> usize {
    let mut attempts = 0_usize;
    for entry in entries {
        if attempts >= max_attempts {
            break;
        }
        // Shared with the store calls below instead of copied into them.
        let original_html: Arc<str> = Arc::from(entry.content_html);
        let Some(expected_content_hash) = compute_entry_content_hash(
            Some(&original_html),
            entry.content_text.as_deref(),
            Some(&entry.title),
            entry.url.as_ref(),
        ) else {
            continue;
        };
        match entry_repository
            .has_derived_body(
                feed_id,
                entry.dedup_key.clone(),
                expected_content_hash.clone(),
                Arc::clone(&original_html),
            )
            .await
        {
            Ok(true) => continue,
            Ok(false) => {}
            Err(error) => {
                tracing::warn!(
                    feed_id,
                    dedup_key = %entry.dedup_key,
                    error = %error,
                    "读取已存正文失败，跳过后台图片本地化"
                );
                continue;
            }
        }

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
                    entry_url = ?entry.url,
                    reason = "timeout",
                    timeout_secs = localization_timeout(localizer).as_secs(),
                    "后台正文图片本地化超时，跳过当前文章"
                );
                continue;
            }
        };

        let update = LocalizedBody {
            dedup_key: entry.dedup_key.clone(),
            expected_content_hash: Some(expected_content_hash),
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
mod tests {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::Arc,
        thread,
    };

    use crate::store::{LibraryEntries, LibraryFeeds, LibraryRefreshStore, LibraryStore};
    use rssr_application::{
        FeedRefreshUpdate, ParsedEntryData, ParsedFeedUpdate, RefreshCommit, RefreshStorePort,
    };
    use rssr_domain::{FeedRepository, NewFeedSubscription};
    use url::Url;

    use super::{BodyAssetLocalizer, ImageLocalizationWorker, localize_feed_entries};

    /// An in-memory library with one subscription and one entry written by a refresh.
    async fn library_with_entry(url: Url, html: &str) -> (Arc<LibraryEntries>, i64) {
        let store = LibraryStore::open_in_memory().await.expect("open library");
        let feed = LibraryFeeds::new(store.clone())
            .upsert_subscription(&NewFeedSubscription {
                site_url: None,
                url: Url::parse("https://example.com/feed.xml").expect("feed url"),
                title: Some("Example".to_string()),
                source_title: None,
                folder: None,
            })
            .await
            .expect("create feed");
        let refresh = LibraryRefreshStore::new(store.clone());
        let target = refresh.get_target(feed.id).await.expect("target").expect("active feed");
        let entry = ParsedEntryData {
            external_id: "entry-1".to_string(),
            dedup_key: "entry-1".to_string(),
            url: Some(url),
            title: "Entry".to_string(),
            author: None,
            summary: Some("summary".to_string()),
            content_html: Some(html.to_string()),
            content_text: None,
            published_at: None,
            updated_at_source: None,
        };
        let update = FeedRefreshUpdate {
            metadata: Default::default(),
            feed: ParsedFeedUpdate {
                title: None,
                site_url: None,
                description: None,
                entries: vec![entry],
            },
        };
        refresh.commit(&target, RefreshCommit::Updated { update }).await.expect("insert entry");
        (Arc::new(LibraryEntries::new(store)), feed.id)
    }

    #[tokio::test]
    async fn localize_entry_on_demand_rewrites_current_entry_html() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind local image server");
        let addr = listener.local_addr().expect("listener addr");
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept image request");
            let mut buf = [0_u8; 4096];
            let _ = stream.read(&mut buf).expect("read request");
            let response = concat!(
                "HTTP/1.1 200 OK\r\n",
                "Connection: close\r\n",
                "Content-Type: image/png\r\n",
                "Content-Length: 4\r\n\r\n"
            );
            stream.write_all(response.as_bytes()).expect("write headers");
            stream.write_all(&[1_u8, 2_u8, 3_u8, 4_u8]).expect("write image bytes");
            stream.flush().expect("flush response");
        });

        let article_url = Url::parse(&format!("http://{addr}/article")).expect("article url");
        let (entry_repository, _) =
            library_with_entry(article_url, r#"<p><img src="/hero.png" alt="hero"></p>"#).await;

        let worker = ImageLocalizationWorker::new(entry_repository.clone());

        let updated = worker.localize_entry_on_demand(1).await.expect("localize current entry");
        assert!(updated, "expected current reader entry to be rewritten");

        let entry = entry_repository
            .get_entry(1)
            .await
            .expect("load localized entry")
            .expect("entry exists");
        let html = entry.content_html.expect("localized html");
        assert!(html.contains("data:image/png;base64,"));
        assert!(!html.contains("/hero.png"));

        server.join().expect("join local image server");
    }

    /// 后台本地化按尝试计数（审计 C2）：图片全部失败（例如防盗链 403）时，一轮最多为
    /// `max_attempts` 篇发请求，而不是把订阅里的每一篇都试一遍；排在前面的无图文章不发请求，
    /// 也不占用尝试次数。
    #[tokio::test]
    async fn background_localization_stops_after_its_attempt_budget() {
        let (addr, requests, server) = forbidden_image_server().await;

        let store = LibraryStore::open_in_memory().await.expect("open library");
        let entry_repository = LibraryEntries::new(store);
        let article = Url::parse(&format!("http://{addr}/article")).expect("article url");
        let entries = (0..13)
            .map(|index| rssr_application::RefreshLocalizedEntry {
                generation: 0,
                dedup_key: format!("entry-{index}"),
                url: Some(article.clone()),
                title: format!("Entry {index}"),
                content_html: if index < 5 {
                    format!("<p>Text only {index}</p>")
                } else {
                    format!(r#"<p><img src="http://{addr}/image-{index}.png"></p>"#)
                },
                content_text: None,
            })
            .collect();

        let attempts =
            localize_feed_entries(&entry_repository, &BodyAssetLocalizer::new(), 1, entries, 3)
                .await;
        server.abort();

        assert_eq!(attempts, 3);
        assert_eq!(requests.load(std::sync::atomic::Ordering::SeqCst), 3);
    }

    /// 图片全部失败时，正文只是被规范化（相对地址补全等），不写回：否则这篇文章会被当成已经
    /// 本地化，之后的刷新不再重试（独立评审）。
    #[tokio::test]
    async fn failed_background_localization_leaves_the_source_body_for_a_retry() {
        let (addr, requests, server) = forbidden_image_server().await;
        let article = Url::parse(&format!("http://{addr}/posts/1")).expect("article url");
        let source_html = r#"<p><img src="/image.png"></p>"#;
        let (entry_repository, feed_id) = library_with_entry(article.clone(), source_html).await;
        let (_, generation) =
            entry_repository.active_entry_generation(1).await.expect("generation").expect("active");
        let entry = rssr_application::RefreshLocalizedEntry {
            generation,
            dedup_key: "entry-1".to_string(),
            url: Some(article.clone()),
            title: "Entry".to_string(),
            content_html: source_html.to_string(),
            content_text: None,
        };

        let attempts = localize_feed_entries(
            &entry_repository,
            &BodyAssetLocalizer::new(),
            feed_id,
            vec![entry],
            5,
        )
        .await;
        server.abort();

        assert_eq!((attempts, requests.load(std::sync::atomic::Ordering::SeqCst)), (1, 1));
        let stored = entry_repository.get_entry(1).await.expect("load").expect("entry");
        assert_eq!(stored.content_html.as_deref(), Some(source_html));
    }

    /// 本地图片服务器：对每个请求都回 403，并记下请求数。
    async fn forbidden_image_server()
    -> (std::net::SocketAddr, Arc<std::sync::atomic::AtomicUsize>, tokio::task::JoinHandle<()>)
    {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("listener addr");
        let requests = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let server = tokio::spawn({
            let requests = Arc::clone(&requests);
            async move {
                loop {
                    let Ok((mut stream, _)) = listener.accept().await else { return };
                    requests.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    let mut buf = [0_u8; 4096];
                    let _ = tokio::io::AsyncReadExt::read(&mut stream, &mut buf).await;
                    let _ = tokio::io::AsyncWriteExt::write_all(
                        &mut stream,
                        b"HTTP/1.1 403 Forbidden\r\nConnection: close\r\nContent-Length: 0\r\n\r\n",
                    )
                    .await;
                }
            }
        });
        (addr, requests, server)
    }
}
