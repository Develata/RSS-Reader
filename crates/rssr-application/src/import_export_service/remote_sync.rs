//! Automatic synchronization of the configuration with a remote store
//! (`docs/design/webdav-auto-sync.md`): the saved profile, the base of the last synchronization,
//! and the rule that picks a direction without ever merging.
//!
//! Two digests take part. The local [`ConfigFingerprint`] covers subscriptions by their local id,
//! so it is compared on this library only; a refresh that renames a source does not change it.
//! A [`RemoteContentHash`] covers a configuration package's content (subscriptions ordered by
//! address, settings; not the export time), so it is comparable across devices.

use anyhow::Result;
use rssr_domain::{ConfigFeed, ConfigFingerprint, ConfigPackage, DomainError, UserSettings};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use time::OffsetDateTime;

use super::rules::validate_config_package;
use super::{
    ConditionalUpload, ImportExportService, RemoteConfigPullOutcome, RemoteConfigPushOutcome,
    RemoteConfigStore, UploadPrecondition,
};

/// Where an upload or a download goes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RemoteTarget {
    /// The endpoint saved for automatic synchronization, as the user saw it: refused when the
    /// saved one is another by now. A success becomes the new base.
    Saved(SavedRemote),
    /// An endpoint given for this operation only; the saved profile is not touched.
    Endpoint(RemoteEndpoint),
}

/// An endpoint as the user typed it (credentials included) and the document's path in it.
/// `Debug` shows no password.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteEndpoint {
    pub endpoint: String,
    pub remote_path: String,
}

impl std::fmt::Debug for RemoteEndpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteEndpoint")
            .field("endpoint", &display_endpoint(&self.endpoint))
            .field("remote_path", &self.remote_path)
            .finish()
    }
}

/// Which saved endpoint the user meant: the endpoint without its password (as
/// [`RemoteSyncProfileView::endpoint`] shows it) and the path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedRemote {
    pub endpoint: String,
    pub remote_path: String,
}

impl SavedRemote {
    /// Whether `profile` is still the endpoint the user saw.
    pub fn is(&self, profile: &RemoteSyncProfile) -> bool {
        display_endpoint(&profile.endpoint) == self.endpoint
            && profile.remote_path == self.remote_path
    }
}

/// The SHA-256 of a configuration package's content, in lower-case hex.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RemoteContentHash(String);

impl RemoteContentHash {
    /// The digest of `package`'s subscriptions (ordered by address, then name and group) and
    /// settings. The version and the export time do not count: two exports of the same
    /// configuration are equal.
    pub fn of(package: &ConfigPackage) -> Self {
        #[derive(Serialize)]
        struct Content<'a> {
            feeds: Vec<&'a ConfigFeed>,
            settings: &'a UserSettings,
        }
        let mut feeds: Vec<&ConfigFeed> = package.feeds.iter().collect();
        feeds.sort_by(|left, right| {
            (&left.url, &left.title, &left.folder).cmp(&(&right.url, &right.title, &right.folder))
        });
        // Plain data with string keys: serializing it cannot fail.
        let json = serde_json::to_vec(&Content { feeds, settings: &package.settings })
            .expect("serialize configuration content");
        Self(Sha256::digest(json).iter().map(|byte| format!("{byte:02x}")).collect())
    }
}

/// The two sides as they were after the last successful synchronization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteSyncBase {
    /// This library's configuration then.
    pub local: ConfigFingerprint,
    /// The remote document's content then.
    pub remote: RemoteContentHash,
}

/// What started a synchronization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteSyncTrigger {
    /// After the first periodic refresh of this process.
    Startup,
    /// After an operation that may have changed the configuration.
    AfterChange,
    /// The user's upload or download with the saved endpoint.
    Manual,
}

/// What a synchronization did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RemoteSyncOutcome {
    /// Nothing to transfer.
    UpToDate,
    Pushed {
        exported_feed_count: u64,
    },
    Pulled {
        imported_feed_count: u64,
        removed_feed_count: u64,
    },
    /// No base yet and the two configurations differ: the user picks the direction once.
    NeedsChoice,
    /// Both sides changed since the base, or one changed during the transfer: nothing was
    /// transferred; the user picks the direction.
    Conflict,
    /// The synchronization failed (most often: offline); the message is the user's.
    Failed {
        message: String,
    },
}

/// The last synchronization's result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteSyncReport {
    pub at: OffsetDateTime,
    pub trigger: RemoteSyncTrigger,
    /// What the synchronization did (a download that happened stays a download).
    pub outcome: RemoteSyncOutcome,
    /// Its base could not be stored: automatic synchronization goes on from the old one.
    #[serde(default)]
    pub unrecorded: bool,
}

/// The endpoint saved for automatic synchronization, kept by the host next to the library and
/// never inside it, nor in a configuration package. `Debug` shows no password.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteSyncProfile {
    /// As the user entered it, credentials included.
    pub endpoint: String,
    pub remote_path: String,
    #[serde(default)]
    pub base: Option<RemoteSyncBase>,
    #[serde(default)]
    pub last_report: Option<RemoteSyncReport>,
}

impl RemoteSyncProfile {
    /// A profile without a base: its first synchronization decides from scratch.
    pub fn new(endpoint: impl Into<String>, remote_path: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            remote_path: remote_path.into(),
            base: None,
            last_report: None,
        }
    }

    /// Which saved endpoint this is, as the user sees it.
    pub fn saved_remote(&self) -> SavedRemote {
        SavedRemote {
            endpoint: display_endpoint(&self.endpoint),
            remote_path: self.remote_path.clone(),
        }
    }

    /// What the user interface may see: no password.
    pub fn view(&self) -> RemoteSyncProfileView {
        RemoteSyncProfileView {
            endpoint: display_endpoint(&self.endpoint),
            remote_path: self.remote_path.clone(),
            has_base: self.base.is_some(),
            last_report: self.last_report.clone(),
        }
    }
}

impl std::fmt::Debug for RemoteSyncProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteSyncProfile")
            .field("endpoint", &display_endpoint(&self.endpoint))
            .field("remote_path", &self.remote_path)
            .field("base", &self.base)
            .field("last_report", &self.last_report)
            .finish()
    }
}

/// A saved profile as the user interface sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteSyncProfileView {
    /// The endpoint without its password.
    pub endpoint: String,
    pub remote_path: String,
    /// Whether a synchronization succeeded since the profile was saved.
    pub has_base: bool,
    pub last_report: Option<RemoteSyncReport>,
}

/// Automatic synchronization as the user interface sees it.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct RemoteSyncView {
    /// Whether this backend can keep a profile (not the Pages demo).
    pub available: bool,
    pub profile: Option<RemoteSyncProfileView>,
    /// A synchronization is scheduled or running: its result is not in `profile` yet.
    #[serde(default)]
    pub syncing: bool,
    /// The saved profile cannot be read; forgetting it is the way out.
    #[serde(default)]
    pub problem: Option<String>,
}

/// `endpoint` without the password of its user information; empty when it is no URL.
pub fn display_endpoint(endpoint: &str) -> String {
    match url::Url::parse(endpoint) {
        Ok(mut url) => {
            let _ = url.set_password(None);
            url.to_string()
        }
        Err(_) => String::new(),
    }
}

/// Where a host keeps the profile.
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
pub trait RemoteSyncProfileStore: Send + Sync {
    async fn load(&self) -> Result<Option<RemoteSyncProfile>>;
    /// Replaces the profile as a whole.
    async fn save(&self, profile: &RemoteSyncProfile) -> Result<()>;
    /// Removes the profile and its credentials; no profile is no error.
    async fn clear(&self) -> Result<()>;
}

/// One synchronization's result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteSyncStep {
    pub outcome: RemoteSyncOutcome,
    /// The base to keep from now on; `None` keeps the current one.
    pub base: Option<RemoteSyncBase>,
}

impl RemoteSyncStep {
    fn new(outcome: RemoteSyncOutcome, base: Option<RemoteSyncBase>) -> Self {
        Self { outcome, base }
    }
}

/// A remote document as a checked configuration package.
fn decode_remote(raw: &str) -> Result<ConfigPackage> {
    let package: ConfigPackage = serde_json::from_str(raw)
        .map_err(|error| DomainError::InvalidInput(format!("远端配置不是有效的配置包：{error}")))?;
    validate_config_package(&package)?;
    Ok(package)
}

fn is_conflict(error: &anyhow::Error) -> bool {
    error
        .chain()
        .any(|cause| matches!(cause.downcast_ref::<DomainError>(), Some(DomainError::Conflict(_))))
}

impl ImportExportService {
    /// One automatic synchronization against `remote`, from `base` (the design's rule table):
    ///
    /// - no remote document: upload it (only while there still is none);
    /// - the same content on both sides: nothing to transfer, it becomes the base;
    /// - otherwise, without a base: [`RemoteSyncOutcome::NeedsChoice`];
    /// - only the remote changed since the base: download and replace (refused, as a conflict,
    ///   when the configuration changes meanwhile);
    /// - only this library changed: upload (only while the remote keeps the entity tag read);
    /// - both changed: [`RemoteSyncOutcome::Conflict`].
    ///
    /// `None` when it does nothing without asking the remote: after a change, while the
    /// configuration still equals the base or there is no base yet.
    pub async fn sync_remote_config(
        &self,
        remote: &dyn RemoteConfigStore,
        base: Option<&RemoteSyncBase>,
        trigger: RemoteSyncTrigger,
    ) -> Result<Option<RemoteSyncStep>> {
        // Only to skip the network: the decision below uses the snapshot read after the download.
        if trigger == RemoteSyncTrigger::AfterChange {
            let local = self.config_fingerprint().await?;
            if base.is_none_or(|base| base.local == local) {
                return Ok(None);
            }
        }
        let document = remote.download_document().await?;
        // The package and its fingerprint from one read: a base never pairs the fingerprint of
        // one configuration with the content of another (independent review, finding 1).
        let (package, local) = self.export_config_with_fingerprint().await?;
        let local_content = RemoteContentHash::of(&package);
        let Some(document) = document else {
            let step = self
                .upload_tracked(remote, local, &package, local_content, UploadPrecondition::Absent)
                .await?;
            return Ok(Some(step));
        };
        let remote_package = decode_remote(&document.raw)?;
        let remote_content = RemoteContentHash::of(&remote_package);
        if remote_content == local_content {
            let base = RemoteSyncBase { local, remote: remote_content };
            return Ok(Some(RemoteSyncStep::new(RemoteSyncOutcome::UpToDate, Some(base))));
        }
        let Some(base) = base else {
            return Ok(Some(RemoteSyncStep::new(RemoteSyncOutcome::NeedsChoice, None)));
        };
        let local_changed = base.local != local;
        let remote_changed = base.remote != remote_content;
        let step = match (local_changed, remote_changed) {
            // Only names a refresh took from the sources differ: no configuration changed.
            (false, false) => RemoteSyncStep::new(RemoteSyncOutcome::UpToDate, None),
            (true, false) => {
                let precondition =
                    document.etag.map_or(UploadPrecondition::None, UploadPrecondition::Matches);
                self.upload_tracked(remote, local, &package, local_content, precondition).await?
            }
            (false, true) => self.replace_tracked(&remote_package, &local, remote_content).await?,
            (true, true) => RemoteSyncStep::new(RemoteSyncOutcome::Conflict, None),
        };
        Ok(Some(step))
    }

    /// Uploads this library's configuration unconditionally (the user's choice; also how a
    /// conflict is resolved); the upload becomes the base.
    pub async fn push_remote_config_tracked(
        &self,
        remote: &dyn RemoteConfigStore,
    ) -> Result<(RemoteConfigPushOutcome, RemoteSyncBase)> {
        let (package, local) = self.export_config_with_fingerprint().await?;
        let raw = serde_json::to_string_pretty(&package)?;
        remote.upload_config(&raw).await?;
        let base = RemoteSyncBase { local, remote: RemoteContentHash::of(&package) };
        Ok((RemoteConfigPushOutcome { exported_feed_count: package.feeds.len() as u64 }, base))
    }

    /// Downloads the configuration and replaces this library's with it, as
    /// [`Self::pull_remote_config`] does; a replacement becomes the base.
    pub async fn pull_remote_config_tracked(
        &self,
        remote: &dyn RemoteConfigStore,
    ) -> Result<(RemoteConfigPullOutcome, Option<RemoteSyncBase>)> {
        let expected = self.config_fingerprint().await?;
        let Some(raw) = remote.download_config().await? else {
            return Ok((RemoteConfigPullOutcome::not_found(), None));
        };
        let package = decode_remote(&raw)?;
        let (imported, fingerprint) = self.replace_with_package(&package, &expected).await?;
        let base = RemoteSyncBase { local: fingerprint, remote: RemoteContentHash::of(&package) };
        Ok((RemoteConfigPullOutcome::imported(imported), Some(base)))
    }

    async fn upload_tracked(
        &self,
        remote: &dyn RemoteConfigStore,
        local: ConfigFingerprint,
        package: &ConfigPackage,
        content: RemoteContentHash,
        precondition: UploadPrecondition,
    ) -> Result<RemoteSyncStep> {
        let raw = serde_json::to_string_pretty(package)?;
        Ok(match remote.upload_config_if(&raw, &precondition).await? {
            ConditionalUpload::Stored => RemoteSyncStep::new(
                RemoteSyncOutcome::Pushed { exported_feed_count: package.feeds.len() as u64 },
                Some(RemoteSyncBase { local, remote: content }),
            ),
            ConditionalUpload::PreconditionFailed => {
                RemoteSyncStep::new(RemoteSyncOutcome::Conflict, None)
            }
        })
    }

    async fn replace_tracked(
        &self,
        package: &ConfigPackage,
        expected: &ConfigFingerprint,
        content: RemoteContentHash,
    ) -> Result<RemoteSyncStep> {
        match self.replace_with_package(package, expected).await {
            Ok((imported, fingerprint)) => Ok(RemoteSyncStep::new(
                RemoteSyncOutcome::Pulled {
                    imported_feed_count: imported.imported_feed_count,
                    removed_feed_count: imported.removed_feed_count,
                },
                Some(RemoteSyncBase { local: fingerprint, remote: content }),
            )),
            Err(error) if is_conflict(&error) => {
                Ok(RemoteSyncStep::new(RemoteSyncOutcome::Conflict, None))
            }
            Err(error) => Err(error),
        }
    }
}
