//! Resolve Android's persistent directory before any database or shell-prefs access.
//! Relocation copies a quiescent previous process's files (including SQLite WAL),
//! verifies the copy, then publishes the whole directory with one rename.
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, ensure};
use sha2::{Digest, Sha256};

use super::ensure_dir as create_private_dir;

fn local_data_dir_in_base_dir(base: &Path) -> PathBuf {
    base.join("RSS-Reader")
}

const MARKER: &str = ".android-storage-v1";
const COMPLETE: &[u8] = b"rss-reader Android filesDir v1\n";
type Manifest = BTreeMap<PathBuf, [u8; 32]>;

#[cfg(target_os = "android")]
pub(super) fn data_dir() -> anyhow::Result<PathBuf> {
    use std::sync::{Mutex, OnceLock};
    static READY: OnceLock<PathBuf> = OnceLock::new();
    static INITIALIZING: Mutex<()> = Mutex::new(());
    if let Some(path) = READY.get() {
        return Ok(path.clone());
    }
    let _guard =
        INITIALIZING.lock().map_err(|_| anyhow::anyhow!("Android 数据目录初始化锁已损坏"))?;
    if let Some(path) = READY.get() {
        return Ok(path.clone());
    }
    let files = files_dir()?;
    // Exactly the previous resolver's HOME / temp fallback, for upgrade discovery only.
    let legacy_base = std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(std::env::temp_dir);
    let path = relocate(&files, &local_data_dir_in_base_dir(&legacy_base))?;
    let _ = READY.set(path.clone());
    Ok(path)
}

#[cfg(target_os = "android")]
fn files_dir() -> anyhow::Result<PathBuf> {
    use jni::{
        JavaVM,
        objects::{JObject, JString},
    };
    let context = ndk_context::android_context();
    // VM and Activity references are owned by the Android host.
    let vm = unsafe { JavaVM::from_raw(context.vm().cast()) }.context("无法访问 Android JavaVM")?;
    let mut env = vm.attach_current_thread().context("无法访问 Android 持久目录")?;
    let activity = unsafe { JObject::from_raw(context.context().cast()) };
    let result = env.with_local_frame(4, |env| -> jni::errors::Result<String> {
        let dir = env.call_method(&activity, "getFilesDir", "()Ljava/io/File;", &[])?.l()?;
        let path = JString::from(
            env.call_method(dir, "getAbsolutePath", "()Ljava/lang/String;", &[])?.l()?,
        );
        Ok(env.get_string(&path)?.into())
    });
    if result.is_err() {
        let _ = env.exception_clear();
    }
    let path = PathBuf::from(result.context("Android 未提供持久 files 目录")?);
    ensure!(path.is_absolute(), "Android files 目录必须是绝对路径");
    Ok(path)
}

fn relocate(files: &Path, legacy: &Path) -> anyhow::Result<PathBuf> {
    ensure!(files.is_absolute() && legacy.is_absolute(), "Android 数据目录必须是绝对路径");
    create_private_dir(files)?;
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(files.join(".rssr-storage-migration.lock"))?;
    // Serializes concurrent startup callers/processes; a process exit releases it.
    lock.lock().context("无法锁定 Android 数据目录迁移")?;
    let destination = local_data_dir_in_base_dir(files);
    if destination.try_exists()? {
        ensure!(fs::symlink_metadata(&destination)?.is_dir(), "Android 数据目标不是普通目录");
        if fs::read(destination.join(MARKER)).ok().as_deref() == Some(COMPLETE) {
            return Ok(destination);
        }
        // An old HOME already under filesDir needs no copying, but still needs a marker.
        if legacy.try_exists()? && fs::canonicalize(legacy)? == fs::canonicalize(&destination)? {
            write_marker(&destination)?;
            sync_dir(&destination)?;
            return Ok(destination);
        }
        anyhow::bail!(
            "Android 持久目录已存在但没有完成标记；保留两处数据并停止迁移: {}",
            destination.display()
        );
    }
    let before = manifest(legacy)?;
    ensure!(
        !before.contains_key(Path::new("rss-reader-content.db"))
            || before.contains_key(Path::new("rss-reader.db")),
        "旧目录缺少索引库，保留旧数据并停止迁移"
    );
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let staging = files.join(format!(".rssr-storage-{}-{stamp}.pending", std::process::id()));
    // Do not reuse an interrupted copy as live data. Its original remains untouched;
    // a retry builds a new verified copy, and publishes only when complete.
    ensure!(!staging.try_exists()?, "Android 迁移暂存路径已存在");
    create_private_dir(&staging)?;
    for relative in before.keys() {
        let target = staging.join(relative);
        create_private_dir(target.parent().context("暂存文件缺少父目录")?)?;
        fs::copy(legacy.join(relative), &target)
            .with_context(|| format!("复制 Android 数据失败: {}", relative.display()))?;
        OpenOptions::new().write(true).open(&target)?.sync_all()?;
    }
    ensure!(manifest(&staging)? == before, "Android 数据副本校验失败；旧数据未改动");
    ensure!(manifest(legacy)? == before, "迁移期间旧数据发生变化；未切换目录，请重启重试");
    write_marker(&staging)?;
    sync_tree(&staging)?;
    ensure!(!destination.try_exists()?, "Android 持久目录在迁移期间被创建，未覆盖");
    fs::rename(&staging, &destination).context("发布 Android 持久数据目录失败")?;
    sync_dir(files)?;
    tracing::info!(source=%legacy.display(), destination=%destination.display(), files=before.len(),
        "Android 数据目录已迁入持久存储，旧目录保留");
    Ok(destination)
}

fn write_marker(directory: &Path) -> anyhow::Result<()> {
    let mut marker =
        OpenOptions::new().write(true).create_new(true).open(directory.join(MARKER))?;
    marker.write_all(COMPLETE)?;
    marker.sync_all()?;
    Ok(())
}

fn manifest(directory: &Path) -> anyhow::Result<Manifest> {
    let mut result = Manifest::new();
    if directory.try_exists()? {
        ensure!(fs::symlink_metadata(directory)?.is_dir(), "数据源不是普通目录");
        collect(directory, Path::new(""), &mut result)?;
    }
    Ok(result)
}

fn collect(root: &Path, relative: &Path, result: &mut Manifest) -> anyhow::Result<()> {
    for entry in fs::read_dir(root.join(relative))? {
        let entry = entry?;
        let path = relative.join(entry.file_name());
        let kind = entry.file_type()?;
        ensure!(!kind.is_symlink(), "数据目录含符号链接，停止自动迁移: {}", path.display());
        if kind.is_dir() {
            collect(root, &path, result)?;
        } else {
            ensure!(kind.is_file(), "数据目录含非常规文件: {}", path.display());
            // SHM and the old migration lock are process coordination, not persisted data.
            // WAL must be copied: it may contain the last committed transactions.
            if matches!(
                path.to_str(),
                Some(
                    "library.sqlite3-shm"
                        | "rss-reader.db-shm"
                        | "rss-reader-content.db-shm"
                        | ".rss-reader.db.migration.lock"
                        | MARKER
                )
            ) {
                continue;
            }
            let mut file = File::open(entry.path())?;
            let mut digest = Sha256::new();
            let mut buffer = [0u8; 65536];
            loop {
                let read = file.read(&mut buffer)?;
                if read == 0 {
                    break;
                }
                digest.update(&buffer[..read]);
            }
            result.insert(path, digest.finalize().into());
        }
    }
    Ok(())
}

fn sync_tree(directory: &Path) -> anyhow::Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            sync_tree(&entry.path())?;
        }
    }
    sync_dir(directory)
}

fn sync_dir(directory: &Path) -> anyhow::Result<()> {
    #[cfg(unix)]
    File::open(directory)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = directory;
    Ok(())
}

#[cfg(test)]
mod tests;
