//! Where the desktop app, Android and the CLI keep their library, and opening it.
//!
//! Location rules (one place for the library and the interface preferences next to it):
//! - Linux: installed under `/usr/bin` -> `$XDG_DATA_HOME/rss-reader` (default
//!   `~/.local/share/rss-reader`); otherwise portable, next to the executable (`RSS-Reader/`);
//! - macOS: `~/Library/Application Support/RSS-Reader` (an app bundle is read-only once signed);
//!   v0.1.x kept its data next to the executable inside the bundle, which is where the first
//!   start imports from;
//! - Android: `$HOME/RSS-Reader` (the app's private directory);
//! - other platforms: next to the executable (`RSS-Reader/`).
//!
//! The first open in a data directory without a library imports a v0.1.x library found there
//! (see [`super::import`]). A failed import fails the open: an empty library would hide the old
//! data.

#[cfg(target_os = "linux")]
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use super::error::{StoreError, StoreResult};
use super::executor::{LibraryStore, StoreOptions};
use super::import::{self, ImportOutcome, V01Library};

/// File name of the library in a data directory.
pub const LIBRARY_FILE: &str = "library.sqlite3";

const LOCAL_DATA_DIR_NAME: &str = "RSS-Reader";

fn io_error(message: impl Into<String>) -> StoreError {
    StoreError::Io(std::io::Error::other(message.into()))
}

/// The local data directory of this installation.
pub fn local_data_dir() -> StoreResult<PathBuf> {
    #[cfg(target_os = "android")]
    {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .filter(|path| !path.as_os_str().is_empty())
            .or_else(|| {
                let fallback = std::env::temp_dir();
                (!fallback.as_os_str().is_empty()).then_some(fallback)
            })
            .ok_or_else(|| io_error("Android 环境未提供可写的 HOME 目录"))?;
        Ok(home.join(LOCAL_DATA_DIR_NAME))
    }
    #[cfg(target_os = "macos")]
    {
        macos_data_dir(std::env::var_os("HOME").as_deref())
    }
    #[cfg(not(any(target_os = "android", target_os = "macos")))]
    {
        let executable_dir = executable_dir()?;
        #[cfg(target_os = "linux")]
        {
            linux_data_dir(
                &executable_dir,
                std::env::var_os("XDG_DATA_HOME").as_deref(),
                std::env::var_os("HOME").as_deref(),
            )
        }
        #[cfg(not(target_os = "linux"))]
        {
            Ok(executable_dir.join(LOCAL_DATA_DIR_NAME))
        }
    }
}

/// Where a v0.1.x installation of this platform kept its library: the data directory, except on
/// macOS, where it was next to the executable.
pub fn legacy_data_dir() -> StoreResult<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        Ok(executable_dir()?.join(LOCAL_DATA_DIR_NAME))
    }
    #[cfg(not(target_os = "macos"))]
    {
        local_data_dir()
    }
}

#[cfg(not(target_os = "android"))]
fn executable_dir() -> StoreResult<PathBuf> {
    let executable = std::env::current_exe()?;
    executable.parent().map(Path::to_path_buf).ok_or_else(|| io_error("无法定位可执行文件所在目录"))
}

#[cfg(any(target_os = "macos", all(test, unix)))]
fn macos_data_dir(home: Option<&std::ffi::OsStr>) -> StoreResult<PathBuf> {
    let home = home
        .map(Path::new)
        .filter(|path| path.is_absolute())
        .ok_or_else(|| io_error("macOS 需要绝对路径 HOME 才能保存本地数据"))?;
    Ok(home.join("Library").join("Application Support").join(LOCAL_DATA_DIR_NAME))
}

#[cfg(target_os = "linux")]
fn linux_data_dir(
    executable_dir: &Path,
    xdg_data_home: Option<&OsStr>,
    home: Option<&OsStr>,
) -> StoreResult<PathBuf> {
    if executable_dir != Path::new("/usr/bin") {
        return Ok(executable_dir.join(LOCAL_DATA_DIR_NAME));
    }
    let data_home = xdg_data_home
        .map(Path::new)
        .filter(|path| path.is_absolute())
        .map(Path::to_path_buf)
        .or_else(|| {
            home.map(Path::new)
                .filter(|path| path.is_absolute())
                .map(|path| path.join(".local/share"))
        })
        .ok_or_else(|| {
            io_error("Linux 安装版需要绝对路径 XDG_DATA_HOME 或 HOME 才能保存本地数据")
        })?;
    Ok(data_home.join("rss-reader"))
}

/// Creates `dir` if needed; a new directory is private to the user (existing ones keep their
/// permissions).
pub fn ensure_dir(dir: &Path) -> StoreResult<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new().recursive(true).mode(0o700).create(dir)?;
    }
    #[cfg(not(unix))]
    std::fs::create_dir_all(dir)?;
    Ok(())
}

/// The local data directory, created if needed.
pub fn ensure_local_data_dir() -> StoreResult<PathBuf> {
    let dir = local_data_dir()?;
    ensure_dir(&dir)?;
    Ok(dir)
}

/// An opened library and what its first open imported.
pub struct OpenedLibrary {
    pub store: LibraryStore,
    pub path: PathBuf,
    pub imported: Option<ImportOutcome>,
}

/// Opens the library in `data_dir`. When there is none yet and `legacy_dir` holds a v0.1.x
/// library, imports it first; when another process imports at the same time, opens the library
/// it published.
pub async fn open_library_in(
    data_dir: &Path,
    legacy_dir: &Path,
    options: StoreOptions,
) -> StoreResult<OpenedLibrary> {
    ensure_dir(data_dir)?;
    let path = data_dir.join(LIBRARY_FILE);
    let mut imported = None;
    if !path.exists()
        && let Some(source) = V01Library::find(legacy_dir)
    {
        let target = path.clone();
        let result = run_on_thread(move || import::import_v01(&source, &target)).await;
        match result {
            Ok(outcome) => {
                tracing::info!(?outcome, "已导入 v0.1 资料库");
                imported = Some(outcome);
            }
            // Another process published the library meanwhile: use it.
            Err(_) if path.exists() => {}
            Err(error) => return Err(error),
        }
    }
    let store = LibraryStore::open_with(&path, options).await?;
    Ok(OpenedLibrary { store, path, imported })
}

/// Opens this installation's library (see the module documentation for the locations).
pub async fn open_local_library() -> StoreResult<OpenedLibrary> {
    open_library_in(&local_data_dir()?, &legacy_data_dir()?, StoreOptions::default()).await
}

async fn run_on_thread<R: Send + 'static>(
    body: impl FnOnce() -> StoreResult<R> + Send + 'static,
) -> StoreResult<R> {
    let (reply, receive) = tokio::sync::oneshot::channel();
    std::thread::Builder::new().name("rssr-library-import".to_owned()).spawn(move || {
        let _ = reply.send(body());
    })?;
    receive.await.map_err(|_| StoreError::TaskFailed)?
}

// Every case uses Unix paths or permissions.
#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn macos_keeps_data_in_application_support() {
        assert_eq!(
            macos_data_dir(Some(std::ffi::OsStr::new("/Users/张 三"))).unwrap(),
            Path::new("/Users/张 三/Library/Application Support/RSS-Reader")
        );
        assert!(macos_data_dir(Some(std::ffi::OsStr::new("relative"))).is_err());
        assert!(macos_data_dir(None).is_err());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn installed_linux_uses_xdg_and_portable_linux_keeps_adjacent_data() {
        let installed = Path::new("/usr/bin");
        let xdg = Some(OsStr::new("/home/张 三/数据目录"));
        let home = Some(OsStr::new("/home/张 三"));
        assert_eq!(
            linux_data_dir(installed, xdg, home).unwrap(),
            Path::new("/home/张 三/数据目录/rss-reader")
        );
        assert_eq!(
            linux_data_dir(installed, None, home).unwrap(),
            Path::new("/home/张 三/.local/share/rss-reader")
        );
        for invalid_xdg in ["", "relative/path"] {
            assert_eq!(
                linux_data_dir(installed, Some(OsStr::new(invalid_xdg)), home).unwrap(),
                Path::new("/home/张 三/.local/share/rss-reader")
            );
        }
        assert!(linux_data_dir(installed, None, None).is_err());
        assert!(linux_data_dir(installed, xdg, None).is_ok());
        assert!(linux_data_dir(installed, None, Some(OsStr::new("relative"))).is_err());
        assert_eq!(
            linux_data_dir(Path::new("/home/张 三/portable"), xdg, home).unwrap(),
            Path::new("/home/张 三/portable/RSS-Reader")
        );
    }

    #[test]
    fn a_new_data_directory_is_private_but_existing_permissions_stay() {
        use std::os::unix::fs::PermissionsExt;
        let nonce =
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let base = std::env::temp_dir().join(format!("rssr-目录 {nonce}"));
        let dir = base.join("nested/rss-reader");
        ensure_dir(&dir).unwrap();
        assert_eq!(std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777, 0o700);
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o750)).unwrap();
        ensure_dir(&dir).unwrap();
        assert_eq!(std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777, 0o750);
        std::fs::remove_dir_all(base).unwrap();
    }
}
