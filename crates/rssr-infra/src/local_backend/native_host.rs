//! The host of the desktop app, Android and (later) the server: tokio, WebDAV, and image
//! localization into the library.

use std::path::Path;
use std::time::Duration;

use rssr_application::{
    DEFAULT_REFRESH_CONCURRENCY, RefreshLocalizationCandidate, RemoteConfigStore,
    RemoteSyncProfileStore, SERIAL_REFRESH_CONCURRENCY,
};
use time::OffsetDateTime;

use super::image_localization::ImageLocalizationWorker;
use super::{LocalBackend, LocalHost, LocalTask};
use crate::composition::{
    LibraryComposition, compose_library_use_cases, compose_library_use_cases_with,
};
use crate::config_sync::profile_file::FileRemoteSyncProfiles;
use crate::config_sync::webdav::WebDavConfigSync;
use crate::fetch::BodyAssetLocalizer;
use crate::outbound::OutboundPolicy;
use crate::store::LibraryStore;

/// The backend over an opened library, with this host; outbound requests are unrestricted (the
/// desktop app, Android, an ordinary CLI).
pub fn native_local_backend(store: LibraryStore) -> LocalBackend<NativeHost> {
    backend(unrestricted_parts(store), OutboundPolicy::Unrestricted, None)
}

/// [`native_local_backend`] whose every outbound request (feeds, subscriptions, images, WebDAV)
/// follows `policy`; a client that cannot be built is an error.
pub fn native_local_backend_with(
    store: LibraryStore,
    policy: OutboundPolicy,
) -> anyhow::Result<LocalBackend<NativeHost>> {
    Ok(backend(parts(store, &policy)?, policy, None))
}

/// [`native_local_backend_with`] for the library of `data_dir`, whose WebDAV synchronization
/// profile it keeps there (`remote-sync.json`): the desktop app, Android and the server.
pub fn native_local_backend_in(
    store: LibraryStore,
    policy: OutboundPolicy,
    data_dir: &Path,
) -> anyhow::Result<LocalBackend<NativeHost>> {
    let profiles = FileRemoteSyncProfiles::new(data_dir);
    Ok(backend(parts(store, &policy)?, policy, Some(profiles)))
}

/// The use cases, the image localization worker and the refresh concurrency over `store`.
type Parts = (LibraryComposition, ImageLocalizationWorker, usize);

fn unrestricted_parts(store: LibraryStore) -> Parts {
    let refresh_concurrency = refresh_concurrency(&store);
    let composition = compose_library_use_cases(store);
    let worker = ImageLocalizationWorker::new(
        composition.entries.clone(),
        BodyAssetLocalizer::new(),
        BodyAssetLocalizer::for_reader_entry(),
    );
    (composition, worker, refresh_concurrency)
}

fn parts(store: LibraryStore, policy: &OutboundPolicy) -> anyhow::Result<Parts> {
    if *policy == OutboundPolicy::Unrestricted {
        return Ok(unrestricted_parts(store));
    }
    let refresh_concurrency = refresh_concurrency(&store);
    let composition = compose_library_use_cases_with(store, policy)?;
    let worker = ImageLocalizationWorker::new(
        composition.entries.clone(),
        BodyAssetLocalizer::background(policy.clone())?,
        BodyAssetLocalizer::reader(policy.clone())?,
    );
    Ok((composition, worker, refresh_concurrency))
}

fn backend(
    (composition, image_localization_worker, refresh_concurrency): Parts,
    policy: OutboundPolicy,
    remote_sync_profiles: Option<FileRemoteSyncProfiles>,
) -> LocalBackend<NativeHost> {
    LocalBackend::new(
        composition.use_cases,
        NativeHost { image_localization_worker, refresh_concurrency, policy, remote_sync_profiles },
    )
}

fn refresh_concurrency(store: &LibraryStore) -> usize {
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
    refresh_concurrency
}

/// tokio for background work and time, WebDAV for configuration exchange, and image
/// localization into the library.
pub struct NativeHost {
    image_localization_worker: ImageLocalizationWorker,
    refresh_concurrency: usize,
    policy: OutboundPolicy,
    remote_sync_profiles: Option<FileRemoteSyncProfiles>,
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
        // An endpoint or a path that cannot be used is the user's to correct.
        let invalid =
            |error: anyhow::Error| rssr_domain::DomainError::InvalidInput(format!("{error:#}"));
        let store = WebDavConfigSync::with_policy(endpoint, remote_path, self.policy.clone())
            .map_err(invalid)?;
        store.remote_url().map_err(invalid)?;
        Ok(Box::new(store))
    }

    fn remote_sync_profiles(&self) -> Option<&dyn RemoteSyncProfileStore> {
        self.remote_sync_profiles.as_ref().map(|profiles| profiles as &dyn RemoteSyncProfileStore)
    }

    fn feed_refreshed(&self, feed_id: i64, entries: Vec<RefreshLocalizationCandidate>) {
        let _ = self.image_localization_worker.spawn(feed_id, entries);
    }

    async fn localize_entry_assets(&self, entry_id: i64) -> anyhow::Result<bool> {
        self.image_localization_worker.localize_entry_on_demand(entry_id).await
    }
}
