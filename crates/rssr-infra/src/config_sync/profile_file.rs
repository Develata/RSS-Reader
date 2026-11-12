//! The saved WebDAV synchronization profile of a data directory: `remote-sync.json`, next to the
//! library and never inside it (`docs/design/webdav-auto-sync.md`). The endpoint keeps its
//! credentials in plain text (decided 2026-11-12); the file is owner-only on Unix, like the
//! library, and replaced as a whole through a temporary file and a rename.

use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use rssr_application::{RemoteSyncProfile, RemoteSyncProfileStore};
use serde::{Deserialize, Serialize};

/// The profile's file name in the data directory.
pub const PROFILE_FILE: &str = "remote-sync.json";
/// The file's format; another one is refused rather than guessed.
const FORMAT_VERSION: u32 = 1;
/// A profile is an endpoint, a path, two digests and one result: far below this.
const MAX_PROFILE_BYTES: u64 = 256 * 1024;

#[derive(Serialize, Deserialize)]
struct ProfileFile {
    version: u32,
    #[serde(flatten)]
    profile: RemoteSyncProfile,
}

/// [`RemoteSyncProfileStore`] over `remote-sync.json` in a data directory.
#[derive(Debug, Clone)]
pub struct FileRemoteSyncProfiles {
    directory: PathBuf,
}

impl FileRemoteSyncProfiles {
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self { directory: data_dir.into() }
    }

    fn path(&self) -> PathBuf {
        self.directory.join(PROFILE_FILE)
    }
}

#[async_trait::async_trait]
impl RemoteSyncProfileStore for FileRemoteSyncProfiles {
    async fn load(&self) -> Result<Option<RemoteSyncProfile>> {
        read_profile(&self.path())
    }

    async fn save(&self, profile: &RemoteSyncProfile) -> Result<()> {
        let text = serde_json::to_string_pretty(&ProfileFile {
            version: FORMAT_VERSION,
            profile: profile.clone(),
        })?;
        write_replacing(&self.directory, &self.path(), text.as_bytes())
    }

    async fn clear(&self) -> Result<()> {
        match std::fs::remove_file(self.path()) {
            Err(error) if error.kind() != ErrorKind::NotFound => {
                Err(error).context("删除 WebDAV 同步档案失败")
            }
            _ => Ok(()),
        }
    }
}

fn read_profile(path: &Path) -> Result<Option<RemoteSyncProfile>> {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).context("读取 WebDAV 同步档案失败"),
    };
    let mut text = String::new();
    file.take(MAX_PROFILE_BYTES + 1)
        .read_to_string(&mut text)
        .context("读取 WebDAV 同步档案失败")?;
    if text.len() as u64 > MAX_PROFILE_BYTES {
        bail!("WebDAV 同步档案过大，请停用自动同步后重新设置");
    }
    let file: ProfileFile = serde_json::from_str(&text)
        .map_err(|_| anyhow::anyhow!("WebDAV 同步档案无法识别，请停用自动同步后重新设置"))?;
    if file.version != FORMAT_VERSION {
        bail!("WebDAV 同步档案版本 {} 不受支持，请停用自动同步后重新设置", file.version);
    }
    Ok(Some(file.profile))
}

/// Writes `bytes` to a new owner-only file in `directory`, then renames it over `path`: a reader
/// sees the old profile or the new one, never a part.
fn write_replacing(directory: &Path, path: &Path, bytes: &[u8]) -> Result<()> {
    static ATTEMPT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    std::fs::create_dir_all(directory).context("创建数据目录失败")?;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut created = None;
    for _ in 0..1000 {
        let suffix = ATTEMPT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let temporary =
            directory.join(format!("{PROFILE_FILE}.new-{}-{suffix}", std::process::id()));
        match options.open(&temporary) {
            Ok(file) => {
                created = Some((temporary, file));
                break;
            }
            Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error).context("创建 WebDAV 同步档案临时文件失败"),
        }
    }
    let Some((temporary, mut file)) = created else {
        bail!("无法为 WebDAV 同步档案找到可用的临时文件名");
    };
    let written = file.write_all(bytes).and_then(|()| file.sync_all());
    drop(file);
    let renamed = written.and_then(|()| std::fs::rename(&temporary, path));
    if let Err(error) = renamed {
        let _ = std::fs::remove_file(&temporary);
        return Err(error).context("保存 WebDAV 同步档案失败");
    }
    #[cfg(unix)]
    std::fs::File::open(directory)
        .and_then(|directory| directory.sync_all())
        .context("同步数据目录失败")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use rssr_application::{RemoteSyncOutcome, RemoteSyncReport, RemoteSyncTrigger};

    use super::*;

    fn scratch_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "rssr-remote-sync-{name}-{}-{}",
            std::process::id(),
            time::OffsetDateTime::now_utc().unix_timestamp_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[tokio::test]
    async fn a_saved_profile_reads_back_and_clearing_removes_it() {
        let dir = scratch_dir("roundtrip");
        let store = FileRemoteSyncProfiles::new(&dir);
        assert_eq!(store.load().await.unwrap(), None);

        let mut profile =
            RemoteSyncProfile::new("https://alice:s3cret@dav.example/base/", "config/r.json");
        profile.last_report = Some(RemoteSyncReport {
            at: time::OffsetDateTime::UNIX_EPOCH,
            trigger: RemoteSyncTrigger::Startup,
            outcome: RemoteSyncOutcome::Failed { message: "无法访问远端".into() },
        });
        store.save(&profile).await.unwrap();
        assert_eq!(store.load().await.unwrap(), Some(profile.clone()));
        // Saving again replaces the whole profile.
        profile.last_report = None;
        store.save(&profile).await.unwrap();
        assert_eq!(store.load().await.unwrap(), Some(profile));

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(dir.join(PROFILE_FILE)).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600, "the credentials are owner-only");
        }
        let leftovers = std::fs::read_dir(&dir).unwrap().count();
        assert_eq!(leftovers, 1, "no temporary file stays behind");

        store.clear().await.unwrap();
        assert_eq!(store.load().await.unwrap(), None);
        store.clear().await.expect("clearing nothing is no error");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[tokio::test]
    async fn an_unreadable_profile_is_an_error_that_names_the_way_out() {
        let dir = scratch_dir("corrupt");
        std::fs::write(dir.join(PROFILE_FILE), "{ not json").unwrap();
        let error = FileRemoteSyncProfiles::new(&dir).load().await.unwrap_err();
        assert!(format!("{error:#}").contains("停用自动同步"));

        std::fs::write(
            dir.join(PROFILE_FILE),
            r#"{"version":2,"endpoint":"https://dav.example/","remote_path":"r.json"}"#,
        )
        .unwrap();
        let error = FileRemoteSyncProfiles::new(&dir).load().await.unwrap_err();
        assert!(format!("{error:#}").contains("版本 2"));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
