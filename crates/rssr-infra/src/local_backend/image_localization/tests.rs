use std::sync::{Arc, Mutex};
use std::time::Duration;

use rssr_application::{
    FeedRefreshUpdate, ParsedEntryData, ParsedFeedUpdate, RefreshCommit,
    RefreshLocalizationCandidate, RefreshStorePort,
};
use rssr_domain::{FeedRepository, NewFeedSubscription};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Notify;
use url::Url;

use super::{BodyAssetLocalizer, ImageLocalizationWorker};
use crate::store::{
    LibraryEntries, LibraryFeeds, LibraryRefreshStore, LibraryStore, LocalizedBody,
};

struct Library {
    store: LibraryStore,
    entries: Arc<LibraryEntries>,
    feed_id: i64,
}

impl Library {
    async fn new() -> Self {
        let store = LibraryStore::open_in_memory().await.unwrap();
        let feed = LibraryFeeds::new(store.clone())
            .upsert_subscription(&NewFeedSubscription {
                site_url: None,
                url: Url::parse("https://example.com/feed.xml").unwrap(),
                title: Some("Example".into()),
                source_title: None,
                folder: None,
            })
            .await
            .unwrap();
        Self { entries: Arc::new(LibraryEntries::new(store.clone())), store, feed_id: feed.id }
    }

    async fn commit(&self, entries: Vec<ParsedEntryData>) -> Vec<RefreshLocalizationCandidate> {
        let refresh = LibraryRefreshStore::new(self.store.clone());
        let target = refresh.get_target(self.feed_id).await.unwrap().unwrap();
        let candidates = entries
            .iter()
            .filter_map(|entry| RefreshLocalizationCandidate::from_entry(entry, target.generation))
            .collect();
        refresh
            .commit(
                &target,
                RefreshCommit::Updated {
                    update: FeedRefreshUpdate {
                        metadata: Default::default(),
                        feed: ParsedFeedUpdate {
                            title: None,
                            site_url: None,
                            description: None,
                            entries,
                        },
                    },
                },
            )
            .await
            .unwrap();
        candidates
    }

    fn worker(&self) -> ImageLocalizationWorker {
        ImageLocalizationWorker::new(
            self.entries.clone(),
            BodyAssetLocalizer::new(),
            BodyAssetLocalizer::for_reader_entry(),
        )
    }

    async fn html(&self, id: i64) -> String {
        self.entries.get_entry(id).await.unwrap().unwrap().content_html.unwrap()
    }
}

fn entry(index: usize, url: &Url, html: &str) -> ParsedEntryData {
    ParsedEntryData {
        external_id: format!("entry-{index}"),
        dedup_key: format!("entry-{index}"),
        url: Some(url.clone()),
        title: format!("Entry {index}"),
        author: None,
        summary: None,
        content_html: Some(html.to_owned()),
        content_text: None,
        published_at: None,
        updated_at_source: None,
    }
}

struct ImageServer {
    url: Url,
    requests: Arc<Mutex<Vec<String>>>,
    requested: Arc<Notify>,
    release_slow: Arc<Notify>,
    task: tokio::task::JoinHandle<()>,
}

impl ImageServer {
    async fn start() -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url =
            Url::parse(&format!("http://{}/article", listener.local_addr().unwrap())).unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let requested = Arc::new(Notify::new());
        let release_slow = Arc::new(Notify::new());
        let task = tokio::spawn({
            let requests = Arc::clone(&requests);
            let requested = Arc::clone(&requested);
            let release_slow = Arc::clone(&release_slow);
            async move {
                loop {
                    let (mut stream, _) = listener.accept().await.unwrap();
                    let mut buffer = [0; 4096];
                    let length = stream.read(&mut buffer).await.unwrap();
                    let request = String::from_utf8_lossy(&buffer[..length]);
                    let path = request.split_whitespace().nth(1).unwrap_or_default().to_owned();
                    requests.lock().unwrap().push(path.clone());
                    requested.notify_one();
                    if path == "/slow.png" {
                        release_slow.notified().await;
                    }
                    let response = if matches!(
                        path.as_str(),
                        "/success.png" | "/image-5.png" | "/slow.png"
                    ) {
                        "HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Type: image/png\r\nContent-Length: 4\r\n\r\n1234"
                    } else {
                        "HTTP/1.1 403 Forbidden\r\nConnection: close\r\nContent-Length: 0\r\n\r\n"
                    };
                    let _ = stream.write_all(response.as_bytes()).await;
                }
            }
        });
        Self { url, requests, requested, release_slow, task }
    }

    async fn wait_for_request(&self) {
        tokio::time::timeout(Duration::from_secs(5), self.requested.notified()).await.unwrap();
    }
}

impl Drop for ImageServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[tokio::test]
async fn localize_entry_on_demand_rewrites_current_entry_html() {
    let server = ImageServer::start().await;
    let library = Library::new().await;
    library.commit(vec![entry(0, &server.url, "<img src=/success.png>")]).await;
    assert!(library.worker().localize_entry_on_demand(1).await.unwrap());
    assert!(library.html(1).await.contains("data:image/png;base64,"));
    library.store.close().await.unwrap();
}

#[tokio::test]
async fn failed_first_articles_do_not_starve_later_images() {
    let server = ImageServer::start().await;
    let library = Library::new().await;
    let entries = (0..13)
        .map(|index| {
            let html = if index < 5 {
                "<p>text only</p>".into()
            } else {
                format!("<img src=/image-{}.png>", index - 5)
            };
            entry(index, &server.url, &html)
        })
        .collect();
    let candidates = library.commit(entries).await;
    let worker = library.worker();
    worker.spawn(library.feed_id, candidates.clone()).unwrap().await.unwrap();
    assert_eq!(server.requests.lock().unwrap().len(), 5, "text-only bodies cost no attempts");
    assert!(!server.requests.lock().unwrap().iter().any(|path| path == "/image-5.png"));
    worker.spawn(library.feed_id, candidates.clone()).unwrap().await.unwrap();
    assert_eq!(
        server.requests.lock().unwrap().len(),
        10,
        "each round keeps its five-attempt limit"
    );
    assert!(library.html(11).await.contains("data:image/png;base64,"));
    worker.spawn(library.feed_id, candidates).unwrap().await.unwrap();
    assert_eq!(
        server.requests.lock().unwrap().iter().filter(|path| *path == "/image-5.png").count(),
        1,
        "an already localized body is skipped across subsequent rounds"
    );
    library.store.close().await.unwrap();
}

#[tokio::test]
async fn failed_background_localization_leaves_the_source_for_retry() {
    let server = ImageServer::start().await;
    let library = Library::new().await;
    let html = "<p><img src=/forbidden.png></p>";
    let candidates = library.commit(vec![entry(0, &server.url, html)]).await;
    let worker = library.worker();
    for _ in 0..2 {
        worker.spawn(library.feed_id, candidates.clone()).unwrap().await.unwrap();
        assert_eq!(library.html(1).await, html);
    }
    assert_eq!(server.requests.lock().unwrap().len(), 2);
    library.store.close().await.unwrap();
}

#[tokio::test]
async fn queued_work_reads_current_body_and_checks_url_only_changes() {
    let server = ImageServer::start().await;
    for change_url_only in [false, true] {
        let library = Library::new().await;
        let original = entry(0, &server.url, "<img src=/success.png>");
        let candidates = library.commit(vec![original.clone()]).await;
        let worker = library.worker();
        let slots = worker.background.slots.acquire_many(2).await.unwrap();
        let task = worker.spawn(library.feed_id, candidates).unwrap();
        let mut replacement = original;
        if change_url_only {
            replacement.url = Some(server.url.join("/new-base/article").unwrap());
        } else {
            replacement.content_html = Some("<p>new source</p>".into());
        }
        let expected = replacement.content_html.clone().unwrap();
        library.commit(vec![replacement]).await;
        drop(slots);
        task.await.unwrap();
        assert_eq!(library.html(1).await, expected);
        assert!(server.requests.lock().unwrap().is_empty(), "stale source never starts a request");
        library.store.close().await.unwrap();
    }
}

#[tokio::test]
async fn source_changed_during_request_cannot_be_overwritten() {
    let server = ImageServer::start().await;
    let library = Library::new().await;
    let candidates = library.commit(vec![entry(0, &server.url, "<img src=/slow.png>")]).await;
    let task = library.worker().spawn(library.feed_id, candidates).unwrap();
    server.wait_for_request().await;
    library.commit(vec![entry(0, &server.url, "<p>new source</p>")]).await;
    server.release_slow.notify_one();
    task.await.unwrap();
    assert_eq!(library.html(1).await, "<p>new source</p>");
    library.store.close().await.unwrap();
}

#[tokio::test]
async fn reactivated_subscription_rejects_old_generation_even_with_identical_source() {
    let server = ImageServer::start().await;
    let library = Library::new().await;
    let html = "<img src=/slow.png>";
    let candidates = library.commit(vec![entry(0, &server.url, html)]).await;
    let generation = candidates[0].generation;
    let task = library.worker().spawn(library.feed_id, candidates).unwrap();
    server.wait_for_request().await;
    let feeds = LibraryFeeds::new(library.store.clone());
    feeds.set_deleted(library.feed_id, true).await.unwrap();
    feeds.set_deleted(library.feed_id, false).await.unwrap();
    let replacement = library.commit(vec![entry(0, &server.url, html)]).await;
    assert_ne!(replacement[0].generation, generation);
    server.release_slow.notify_one();
    task.await.unwrap();
    assert_eq!(library.html(1).await, html);
    library.store.close().await.unwrap();
}

#[tokio::test]
async fn reader_derivative_wins_over_slower_background_request() {
    let server = ImageServer::start().await;
    let library = Library::new().await;
    let html = "<img src=/slow.png>";
    let candidates = library.commit(vec![entry(0, &server.url, html)]).await;
    let candidate = candidates[0].clone();
    let task = library.worker().spawn(library.feed_id, candidates).unwrap();
    server.wait_for_request().await;
    let reader_html = "<p>reader-localized version</p>";
    assert!(
        library
            .entries
            .update_localized_body(
                library.feed_id,
                candidate.generation,
                LocalizedBody {
                    dedup_key: candidate.dedup_key,
                    expected_content_hash: Some(candidate.source_content_hash),
                    base_html: Arc::from(html),
                    localized_html: reader_html.into(),
                }
            )
            .await
            .unwrap()
    );
    server.release_slow.notify_one();
    task.await.unwrap();
    assert_eq!(library.html(1).await, reader_html);
    library.store.close().await.unwrap();
}

#[tokio::test]
async fn cancelled_round_releases_admission_and_keeps_progress() {
    let server = ImageServer::start().await;
    let library = Library::new().await;
    let candidates = library
        .commit(vec![
            entry(0, &server.url, "<img src=/slow.png>"),
            entry(1, &server.url, "<img src=/success.png>"),
        ])
        .await;
    let worker = library.worker();
    let task = worker.spawn(library.feed_id, candidates.clone()).unwrap();
    server.wait_for_request().await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    server.release_slow.notify_one();
    // Both candidates are offered again; the second article must be first.
    let next = worker.spawn(library.feed_id, candidates).unwrap();
    // A wraparound may eventually retry the slow first image; release that too.
    server.release_slow.notify_one();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if server.requests.lock().unwrap().len() >= 3 {
                break;
            }
            server.requested.notified().await;
        }
    })
    .await
    .unwrap();
    server.release_slow.notify_one();
    next.await.unwrap();
    assert_eq!(server.requests.lock().unwrap()[1], "/success.png");
    assert!(library.html(2).await.contains("data:image/png;base64,"));
    library.store.close().await.unwrap();
}

#[tokio::test]
async fn localization_read_obeys_utf8_byte_budget_and_generation() {
    let library = Library::new().await;
    let html = "<p>正文</p>";
    let candidates =
        library.commit(vec![entry(0, &Url::parse("https://example.com/").unwrap(), html)]).await;
    let candidate = &candidates[0];
    assert!(
        library
            .entries
            .body_for_localization(
                library.feed_id,
                candidate.generation,
                candidate.dedup_key.clone(),
                html.len()
            )
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        library
            .entries
            .body_for_localization(
                library.feed_id,
                candidate.generation,
                candidate.dedup_key.clone(),
                html.len() - 1
            )
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        library
            .entries
            .body_for_localization(
                library.feed_id,
                candidate.generation + 1,
                candidate.dedup_key.clone(),
                html.len()
            )
            .await
            .unwrap()
            .is_none()
    );
    library.store.close().await.unwrap();
}
