#![cfg(not(target_arch = "wasm32"))]

//! The automatic synchronization's rule table (`docs/design/webdav-auto-sync.md`) against real
//! libraries and a remote store that keeps entity tags: two devices share one remote document.

#[path = "support/library.rs"]
mod library;

use std::sync::{Arc, Mutex};

use library::Library;
use rssr_application::import_export_service::ImportExportService;
use rssr_application::{
    ConditionalUpload, RemoteConfigDocument, RemoteConfigStore, RemoteSyncBase, RemoteSyncOutcome,
    RemoteSyncTrigger, UploadPrecondition,
};
use rssr_domain::{SettingsRepository, UserSettings};
use rssr_infra::{application_adapters::InfraOpmlCodec, opml::OpmlCodec};

/// One remote document with a version-counting entity tag; counts the requests it answers.
#[derive(Default)]
struct Remote {
    state: Mutex<RemoteState>,
}

#[derive(Default)]
struct RemoteState {
    document: Option<(String, u64)>,
    versions: u64,
    downloads: usize,
    uploads: usize,
    /// Replaces the document after the next download, as another device would meanwhile.
    change_after_download: Option<String>,
}

impl Remote {
    fn requests(&self) -> (usize, usize) {
        let state = self.state.lock().unwrap();
        (state.downloads, state.uploads)
    }

    fn raw(&self) -> Option<String> {
        self.state.lock().unwrap().document.as_ref().map(|(raw, _)| raw.clone())
    }

    fn change_after_next_download(&self, raw: String) {
        self.state.lock().unwrap().change_after_download = Some(raw);
    }
}

impl RemoteState {
    fn store(&mut self, raw: String) {
        self.versions += 1;
        self.document = Some((raw, self.versions));
    }
}

#[async_trait::async_trait]
impl RemoteConfigStore for Remote {
    async fn upload_config(&self, raw: &str) -> anyhow::Result<()> {
        let mut state = self.state.lock().unwrap();
        state.uploads += 1;
        state.store(raw.to_owned());
        Ok(())
    }

    async fn download_config(&self) -> anyhow::Result<Option<String>> {
        Ok(self.download_document().await?.map(|document| document.raw))
    }

    async fn download_document(&self) -> anyhow::Result<Option<RemoteConfigDocument>> {
        let mut state = self.state.lock().unwrap();
        state.downloads += 1;
        let document = state.document.as_ref().map(|(raw, version)| RemoteConfigDocument {
            raw: raw.clone(),
            etag: Some(format!("\"{version}\"")),
        });
        if let Some(raw) = state.change_after_download.take() {
            state.store(raw);
        }
        Ok(document)
    }

    async fn upload_config_if(
        &self,
        raw: &str,
        precondition: &UploadPrecondition,
    ) -> anyhow::Result<ConditionalUpload> {
        let mut state = self.state.lock().unwrap();
        state.uploads += 1;
        let current = state.document.as_ref().map(|(_, version)| format!("\"{version}\""));
        let holds = match precondition {
            UploadPrecondition::None => true,
            UploadPrecondition::Absent => current.is_none(),
            UploadPrecondition::Matches(tag) => current.as_ref() == Some(tag),
        };
        if !holds {
            return Ok(ConditionalUpload::PreconditionFailed);
        }
        state.store(raw.to_owned());
        Ok(ConditionalUpload::Stored)
    }
}

/// One device: its library and the base it kept.
struct Device {
    library: Library,
    service: ImportExportService,
    base: Option<RemoteSyncBase>,
}

impl Device {
    async fn new() -> Self {
        let library = Library::memory().await;
        let service = ImportExportService::new(
            Arc::new(library.feeds.clone()),
            Arc::new(library.queries.clone()),
            Arc::new(InfraOpmlCodec::new(OpmlCodec::new())),
            Arc::new(rssr_infra::platform_clock::PlatformClock),
            Arc::new(library.mutations.clone()),
        );
        Self { library, service, base: None }
    }

    /// One synchronization; the base it gives is kept. `None`: it did nothing.
    async fn sync(
        &mut self,
        remote: &Remote,
        trigger: RemoteSyncTrigger,
    ) -> Option<RemoteSyncOutcome> {
        let step = self
            .service
            .sync_remote_config(remote, self.base.as_ref(), trigger)
            .await
            .expect("synchronize")?;
        if let Some(base) = step.base {
            self.base = Some(base);
        }
        Some(step.outcome)
    }

    async fn set_interval(&self, minutes: u32) {
        let settings =
            UserSettings { refresh_interval_minutes: minutes, ..UserSettings::default() };
        self.library.settings.save(&settings).await.expect("save settings");
    }

    async fn interval(&self) -> u32 {
        self.library.settings.load().await.expect("load settings").refresh_interval_minutes
    }

    async fn feed_urls(&self) -> Vec<String> {
        let package = self.service.export_config().await.expect("export");
        let mut urls: Vec<String> = package.feeds.into_iter().map(|feed| feed.url).collect();
        urls.sort();
        urls
    }
}

#[tokio::test]
async fn the_first_device_uploads_and_a_second_with_other_content_must_choose() {
    let remote = Remote::default();
    let mut first = Device::new().await;
    first.library.add_feed("https://a.example/feed.xml").await;

    let pushed = first.sync(&remote, RemoteSyncTrigger::Startup).await;
    assert_eq!(pushed, Some(RemoteSyncOutcome::Pushed { exported_feed_count: 1 }));
    assert!(first.base.is_some(), "the upload is the base");
    assert!(remote.raw().is_some_and(|raw| raw.contains("https://a.example/feed.xml")));

    // Nothing changed on either side: one download, no upload.
    let before = remote.requests();
    assert_eq!(
        first.sync(&remote, RemoteSyncTrigger::Startup).await,
        Some(RemoteSyncOutcome::UpToDate)
    );
    assert_eq!(remote.requests(), (before.0 + 1, before.1));

    // Another library with other content and no base: nothing moves.
    let mut second = Device::new().await;
    second.library.add_feed("https://b.example/feed.xml").await;
    assert_eq!(
        second.sync(&remote, RemoteSyncTrigger::Startup).await,
        Some(RemoteSyncOutcome::NeedsChoice)
    );
    assert!(second.base.is_none());
    assert_eq!(second.feed_urls().await, ["https://b.example/feed.xml"]);
}

#[tokio::test]
async fn the_same_content_without_a_base_becomes_the_base() {
    let remote = Remote::default();
    let mut first = Device::new().await;
    first.library.add_feed("https://a.example/feed.xml").await;
    first.sync(&remote, RemoteSyncTrigger::Startup).await;

    let mut second = Device::new().await;
    second.library.add_feed("https://a.example/feed.xml").await;
    let before = remote.requests();
    assert_eq!(
        second.sync(&remote, RemoteSyncTrigger::Startup).await,
        Some(RemoteSyncOutcome::UpToDate)
    );
    assert!(second.base.is_some(), "equal content is a base");
    assert_eq!(remote.requests().1, before.1, "nothing uploaded");
}

#[tokio::test]
async fn a_change_on_one_device_reaches_the_other_and_a_change_on_both_is_a_conflict() {
    let remote = Remote::default();
    let mut first = Device::new().await;
    first.library.add_feed("https://a.example/feed.xml").await;
    first.sync(&remote, RemoteSyncTrigger::Startup).await;

    // The second device takes the remote once (the user's download), which is its base.
    let mut second = Device::new().await;
    let (_, base) = second.service.pull_remote_config_tracked(&remote).await.expect("pull");
    second.base = base;
    assert_eq!(second.feed_urls().await, ["https://a.example/feed.xml"]);

    // After a change, an unchanged configuration asks nothing of the remote.
    let before = remote.requests();
    assert_eq!(second.sync(&remote, RemoteSyncTrigger::AfterChange).await, None);
    assert_eq!(remote.requests(), before, "no request while the configuration equals the base");

    // The first device changes and uploads after the change.
    first.set_interval(45).await;
    assert_eq!(
        first.sync(&remote, RemoteSyncTrigger::AfterChange).await,
        Some(RemoteSyncOutcome::Pushed { exported_feed_count: 1 })
    );
    // The second downloads it at its next start.
    assert_eq!(
        second.sync(&remote, RemoteSyncTrigger::Startup).await,
        Some(RemoteSyncOutcome::Pulled { imported_feed_count: 1, removed_feed_count: 0 })
    );
    assert_eq!(second.interval().await, 45);
    // Its new base is the replacement: the next start transfers nothing.
    assert_eq!(
        second.sync(&remote, RemoteSyncTrigger::Startup).await,
        Some(RemoteSyncOutcome::UpToDate)
    );

    // Both change: neither side moves.
    first.set_interval(50).await;
    first.sync(&remote, RemoteSyncTrigger::AfterChange).await;
    second.set_interval(55).await;
    let remote_before = remote.raw();
    assert_eq!(
        second.sync(&remote, RemoteSyncTrigger::Startup).await,
        Some(RemoteSyncOutcome::Conflict)
    );
    assert_eq!(second.interval().await, 55);
    assert_eq!(remote.raw(), remote_before);

    // The user resolves it by uploading; the upload is the new base on that side.
    let (_, base) = second.service.push_remote_config_tracked(&remote).await.expect("push");
    second.base = Some(base);
    assert_eq!(
        second.sync(&remote, RemoteSyncTrigger::Startup).await,
        Some(RemoteSyncOutcome::UpToDate)
    );
}

#[tokio::test]
async fn an_upload_after_the_remote_changed_meanwhile_is_a_conflict() {
    let remote = Remote::default();
    let mut first = Device::new().await;
    first.sync(&remote, RemoteSyncTrigger::Startup).await;
    first.set_interval(40).await;

    // Another device uploads between this device's download and its upload.
    let other = Device::new().await;
    other.library.add_feed("https://other.example/feed.xml").await;
    let theirs = other.service.export_config_json().await.unwrap();
    remote.change_after_next_download(theirs.clone());

    assert_eq!(
        first.sync(&remote, RemoteSyncTrigger::AfterChange).await,
        Some(RemoteSyncOutcome::Conflict)
    );
    assert_eq!(remote.raw(), Some(theirs), "the other device's upload stays");
}

#[tokio::test]
async fn a_missing_remote_is_uploaded_again_only_while_it_is_missing() {
    let remote = Remote::default();
    let mut first = Device::new().await;
    first.library.add_feed("https://a.example/feed.xml").await;
    first.sync(&remote, RemoteSyncTrigger::Startup).await;

    // The remote document was deleted: the next start uploads it again.
    remote.state.lock().unwrap().document = None;
    assert_eq!(
        first.sync(&remote, RemoteSyncTrigger::Startup).await,
        Some(RemoteSyncOutcome::Pushed { exported_feed_count: 1 })
    );

    // Created by someone else between the download and the upload: not overwritten.
    remote.state.lock().unwrap().document = None;
    let other = Device::new().await;
    let theirs = other.service.export_config_json().await.unwrap();
    remote.change_after_next_download(theirs.clone());
    assert_eq!(
        first.sync(&remote, RemoteSyncTrigger::Startup).await,
        Some(RemoteSyncOutcome::Conflict)
    );
    assert_eq!(remote.raw(), Some(theirs));
}

#[tokio::test]
async fn an_invalid_remote_document_fails_without_changing_anything() {
    let remote = Remote::default();
    remote.state.lock().unwrap().store("{ not a package".to_owned());
    let device = Device::new().await;
    device.library.add_feed("https://a.example/feed.xml").await;
    let error = device
        .service
        .sync_remote_config(&remote, None, RemoteSyncTrigger::Startup)
        .await
        .expect_err("an invalid document is an error");
    assert!(format!("{error:#}").contains("远端配置不是有效的配置包"), "{error:#}");
    assert_eq!(device.feed_urls().await, ["https://a.example/feed.xml"]);
    assert_eq!(remote.requests().1, 0, "nothing uploaded over it");
}
