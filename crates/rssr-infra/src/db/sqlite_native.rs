use async_trait::async_trait;
#[cfg(target_os = "linux")]
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use anyhow::Context;
use sqlx::sqlite::SqlitePoolOptions;

use crate::db::{
    FILE_SQLITE_MAX_CONNECTIONS, SqlitePool, connect_options_for_path, create_sqlite_pool,
    is_memory_database, migrate, migrate_content, storage_backend::StorageBackend,
};

#[derive(Debug, Clone)]
pub struct NativeSqliteBackend {
    connection: NativeConnection,
}

#[derive(Debug, Clone)]
enum NativeConnection {
    Url(String),
    Path(PathBuf),
}

impl NativeSqliteBackend {
    pub fn new(database_url: impl Into<String>) -> Self {
        Self { connection: NativeConnection::Url(database_url.into()) }
    }

    pub fn with_path(database_path: impl Into<PathBuf>) -> Self {
        Self { connection: NativeConnection::Path(database_path.into()) }
    }

    pub fn from_default_location() -> anyhow::Result<Self> {
        Ok(Self::with_path(ensure_local_data_dir()?.join("rss-reader.db")))
    }

    pub fn database_label(&self) -> String {
        match &self.connection {
            NativeConnection::Url(url) => url.clone(),
            NativeConnection::Path(path) => path.display().to_string(),
        }
    }

    pub fn content_database_label(&self) -> anyhow::Result<String> {
        Ok(match &self.connection {
            NativeConnection::Url(url) => derive_content_database_url(url)?,
            NativeConnection::Path(path) => content_database_path(path).display().to_string(),
        })
    }

    pub async fn connect_content(&self) -> anyhow::Result<SqlitePool> {
        match &self.connection {
            NativeConnection::Url(database_url) => {
                create_sqlite_pool(&derive_content_database_url(database_url)?).await
            }
            NativeConnection::Path(database_path) => {
                let content_path = content_database_path(database_path);
                connect_sqlite_path(&content_path).await
            }
        }
    }

    pub async fn migrate_content(&self, pool: &SqlitePool) -> anyhow::Result<()> {
        migrate_content(pool).await
    }
}

#[async_trait]
impl StorageBackend for NativeSqliteBackend {
    async fn connect(&self) -> anyhow::Result<SqlitePool> {
        match &self.connection {
            NativeConnection::Url(database_url) => create_sqlite_pool(database_url).await,
            NativeConnection::Path(database_path) => connect_sqlite_path(database_path).await,
        }
    }

    async fn migrate(&self, pool: &SqlitePool) -> anyhow::Result<()> {
        migrate(pool).await
    }

    fn label(&self) -> &'static str {
        "sqlite-native"
    }
}

/// 本地可写数据目录：两个数据库和界面偏好始终使用同一解析规则。
///
/// Linux 系统安装包位于 /usr/bin，数据遵循 XDG；便携版仍跟随可执行文件。
/// Android 和其它桌面平台保留原有路径，避免无意拆分已有数据。
pub fn local_data_dir() -> anyhow::Result<PathBuf> {
    #[cfg(target_os = "android")]
    {
        Ok(local_data_dir_in_base_dir(&android_data_base_dir()?))
    }

    #[cfg(not(target_os = "android"))]
    {
        let executable_path = std::env::current_exe().context("无法定位可执行文件路径")?;
        let executable_dir = executable_path.parent().context("无法定位可执行文件所在目录")?;
        #[cfg(target_os = "linux")]
        {
            linux_local_data_dir(
                executable_dir,
                std::env::var_os("XDG_DATA_HOME").as_deref(),
                std::env::var_os("HOME").as_deref(),
            )
        }
        #[cfg(not(target_os = "linux"))]
        {
            Ok(local_data_dir_in_base_dir(executable_dir))
        }
    }
}

/// 创建目录时限制新目录的权限；已有目录及其权限不作更改。
pub fn ensure_local_data_dir() -> anyhow::Result<PathBuf> {
    let dir = local_data_dir()?;
    create_local_data_dir(&dir)?;
    Ok(dir)
}

fn create_local_data_dir(dir: &Path) -> anyhow::Result<()> {
    #[cfg(unix)]
    let result = {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new().recursive(true).mode(0o700).create(dir)
    };
    #[cfg(not(unix))]
    let result = std::fs::create_dir_all(dir);
    result.with_context(|| format!("创建本地数据目录失败: {}", dir.display()))
}

#[cfg(target_os = "linux")]
fn linux_local_data_dir(
    executable_dir: &Path,
    xdg_data_home: Option<&OsStr>,
    home: Option<&OsStr>,
) -> anyhow::Result<PathBuf> {
    if executable_dir != Path::new("/usr/bin") {
        return Ok(local_data_dir_in_base_dir(executable_dir));
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
        .context("Linux 安装版需要绝对路径 XDG_DATA_HOME 或 HOME 才能保存本地数据")?;
    Ok(data_home.join("rss-reader"))
}

#[cfg(target_os = "android")]
fn android_data_base_dir() -> anyhow::Result<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
        .or_else(|| {
            let fallback = std::env::temp_dir();
            (!fallback.as_os_str().is_empty()).then_some(fallback)
        })
        .context("Android 环境未提供可写的 HOME 目录")
}

const LOCAL_DATA_DIR_NAME: &str = "RSS-Reader";

fn local_data_dir_in_base_dir(base_dir: &Path) -> PathBuf {
    base_dir.join(LOCAL_DATA_DIR_NAME)
}

fn content_database_path(index_database_path: &Path) -> PathBuf {
    let parent =
        index_database_path.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."));
    let stem =
        index_database_path.file_stem().and_then(|stem| stem.to_str()).unwrap_or("rss-reader");
    let extension = index_database_path.extension().and_then(|ext| ext.to_str()).unwrap_or("db");

    parent.join(format!("{stem}-content.{extension}"))
}

async fn connect_sqlite_path(database_path: &Path) -> anyhow::Result<SqlitePool> {
    if let Some(parent) = database_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("创建本地数据库目录失败: {}", parent.display()))?;
    }

    // WAL + busy_timeout 由 `connect_options_for_path` 统一设置：并发刷新依赖它。
    let options = connect_options_for_path(database_path);

    SqlitePoolOptions::new()
        .max_connections(FILE_SQLITE_MAX_CONNECTIONS)
        .connect_with(options)
        .await
        .with_context(|| format!("打开本地数据库失败: {}", database_path.display()))
}

fn derive_content_database_url(database_url: &str) -> anyhow::Result<String> {
    if is_memory_database(database_url) {
        return Ok(database_url.to_string());
    }

    let (base, query) = database_url
        .split_once('?')
        .map(|(base, query)| (base, Some(query)))
        .unwrap_or((database_url, None));

    let rewritten = if let Some(path) = base.strip_prefix("sqlite://") {
        format!("sqlite://{}", append_content_suffix(path))
    } else if let Some(path) = base.strip_prefix("sqlite:") {
        format!("sqlite:{}", append_content_suffix(path))
    } else {
        anyhow::bail!("无法推导 content.db 路径：{database_url}");
    };

    Ok(match query {
        Some(query) => format!("{rewritten}?{query}"),
        None => rewritten,
    })
}

fn append_content_suffix(path: &str) -> String {
    let (head, tail) =
        path.rsplit_once('/').map(|(head, tail)| (Some(head), tail)).unwrap_or((None, path));
    let suffixed_tail = if let Some((stem, ext)) = tail.rsplit_once('.') {
        format!("{stem}-content.{ext}")
    } else {
        format!("{tail}-content")
    };

    match head {
        Some(head) => format!("{head}/{suffixed_tail}"),
        None => suffixed_tail,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        NativeSqliteBackend, append_content_suffix, content_database_path, create_local_data_dir,
        local_data_dir_in_base_dir,
    };
    use crate::db::storage_backend::StorageBackend;
    use std::{
        path::Path,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[cfg(target_os = "linux")]
    #[test]
    fn installed_linux_uses_xdg_and_portable_linux_keeps_adjacent_data() {
        use super::linux_local_data_dir;
        use std::ffi::OsStr;

        let installed = Path::new("/usr/bin");
        let xdg = Some(OsStr::new("/home/张 三/数据目录"));
        let home = Some(OsStr::new("/home/张 三"));
        assert_eq!(
            linux_local_data_dir(installed, xdg, home).unwrap(),
            Path::new("/home/张 三/数据目录/rss-reader")
        );
        assert_eq!(
            linux_local_data_dir(installed, None, home).unwrap(),
            Path::new("/home/张 三/.local/share/rss-reader")
        );
        for invalid_xdg in ["", "relative/path"] {
            assert_eq!(
                linux_local_data_dir(installed, Some(OsStr::new(invalid_xdg)), home).unwrap(),
                Path::new("/home/张 三/.local/share/rss-reader")
            );
        }
        assert!(linux_local_data_dir(installed, None, None).is_err());
        assert!(linux_local_data_dir(installed, xdg, None).is_ok());
        assert!(linux_local_data_dir(installed, None, Some(OsStr::new("relative"))).is_err());
        assert_eq!(
            linux_local_data_dir(Path::new("/home/张 三/portable"), xdg, home).unwrap(),
            Path::new("/home/张 三/portable/RSS-Reader")
        );
    }

    #[cfg(unix)]
    #[test]
    fn new_local_data_directory_is_private_but_existing_permissions_stay_unchanged() {
        use std::os::unix::fs::PermissionsExt;

        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos();
        let base = std::env::temp_dir().join(format!("rssr-目录 {nonce}"));
        let data_dir = base.join("nested/rss-reader");
        create_local_data_dir(&data_dir).unwrap();
        assert_eq!(std::fs::metadata(&data_dir).unwrap().permissions().mode() & 0o777, 0o700);
        std::fs::set_permissions(&data_dir, std::fs::Permissions::from_mode(0o750)).unwrap();
        create_local_data_dir(&data_dir).unwrap();
        assert_eq!(std::fs::metadata(&data_dir).unwrap().permissions().mode() & 0o777, 0o750);
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn database_path_uses_project_subdirectory() {
        let path = local_data_dir_in_base_dir(Path::new("/tmp/example")).join("rss-reader.db");
        assert_eq!(path, Path::new("/tmp/example/RSS-Reader/rss-reader.db"));
    }

    /// 数据库与其它本地文件必须落在同一个目录里：`local_data_dir` 是对外的那个入口，
    /// 它一旦和数据库目录漂开，界面偏好就会写到用户找不到、卸载时也清不掉的地方。
    #[test]
    fn local_data_dir_is_the_directory_holding_the_database() {
        let base = Path::new("/tmp/example");
        let data_dir = local_data_dir_in_base_dir(base);

        assert_eq!(data_dir, Path::new("/tmp/example/RSS-Reader"));
        assert_eq!(data_dir.join("rss-reader.db").parent(), Some(data_dir.as_path()));
    }

    #[test]
    fn content_database_path_uses_sibling_file() {
        let path = content_database_path(Path::new("/tmp/example/rss-reader.db"));
        assert_eq!(path, Path::new("/tmp/example/rss-reader-content.db"));
    }

    #[test]
    fn append_content_suffix_preserves_extension() {
        assert_eq!(append_content_suffix("C:/tmp/rss-reader.db"), "C:/tmp/rss-reader-content.db");
    }

    #[test]
    fn content_url_preserves_all_memory_spellings() {
        for url in [
            "sqlite://:memory:",
            ":memory:",
            "sqlite::memory:?cache=shared",
            "sqlite://named?mode=%6demory",
        ] {
            assert_eq!(super::derive_content_database_url(url).unwrap(), url);
        }
    }

    #[tokio::test]
    async fn memory_text_in_filename_keeps_disk_databases_separate() {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let base = std::env::temp_dir().join(format!("rssr-url-中文 {nonce}"));
        std::fs::create_dir_all(&base).unwrap();
        let path = base.join("mode=memory.db");
        let backend = NativeSqliteBackend::new(format!("sqlite://{}?mode=rwc", path.display()));
        let index = backend.connect().await.unwrap();
        let content = backend.connect_content().await.unwrap();
        let index_path: String =
            sqlx::query_scalar("SELECT file FROM pragma_database_list WHERE name='main'")
                .fetch_one(&index)
                .await
                .unwrap();
        let content_path: String =
            sqlx::query_scalar("SELECT file FROM pragma_database_list WHERE name='main'")
                .fetch_one(&content)
                .await
                .unwrap();
        let mode = crate::db::effective_journal_mode(&index).await.unwrap();
        index.close().await;
        content.close().await;
        std::fs::remove_dir_all(base).unwrap();
        assert_ne!(index_path, content_path, "index and content must not share a disk file");
        assert_eq!(mode, "wal");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn filesystem_path_is_not_parsed_as_a_database_url() {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let base = std::env::temp_dir().join(format!("rssr-literal-path-{nonce}"));
        let backend = NativeSqliteBackend::with_path(base.join("literal?mode=memory"));
        let pool = backend.connect().await.unwrap();
        let max_connections = pool.options().get_max_connections();
        let mode = crate::db::effective_journal_mode(&pool).await.unwrap();
        pool.close().await;
        std::fs::remove_dir_all(base).unwrap();
        assert_eq!(max_connections, crate::db::FILE_SQLITE_MAX_CONNECTIONS);
        assert_eq!(mode, "wal");
    }

    #[tokio::test]
    async fn connect_creates_parent_directory_for_database_file() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos();
        let base_dir = std::env::temp_dir().join(format!("rssr-native-backend-{nonce}"));
        let database_path = base_dir.join("nested").join("rss-reader.db");
        let backend = NativeSqliteBackend::with_path(&database_path);

        let pool = backend.connect().await.expect("connect sqlite file");
        backend.migrate(&pool).await.expect("migrate sqlite file");
        pool.close().await;

        assert!(
            database_path.exists(),
            "database file should exist at {}",
            database_path.display()
        );

        let _ = std::fs::remove_file(&database_path);
        let _ = std::fs::remove_file(format!("{}-wal", database_path.display()));
        let _ = std::fs::remove_file(format!("{}-shm", database_path.display()));
        let _ = std::fs::remove_dir_all(base_dir);
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn installed_path_opens_both_databases_under_private_xdg_directory() {
        use super::linux_local_data_dir;

        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos();
        let base = std::env::temp_dir().join(format!("rssr-xdg-中文 {nonce}"));
        let xdg_home = base.join("用户 数据");
        let dir = linux_local_data_dir(
            Path::new("/usr/bin"),
            Some(xdg_home.as_os_str()),
            Some(base.as_os_str()),
        )
        .unwrap();
        create_local_data_dir(&dir).unwrap();
        let backend = NativeSqliteBackend::with_path(dir.join("rss-reader.db"));
        let index = backend.connect().await.unwrap();
        backend.migrate(&index).await.unwrap();
        let content = backend.connect_content().await.unwrap();
        backend.migrate_content(&content).await.unwrap();
        index.close().await;
        content.close().await;
        assert!(dir.join("rss-reader.db").is_file());
        assert!(dir.join("rss-reader-content.db").is_file());

        let reopened = backend.connect().await.unwrap();
        assert!(crate::db::effective_journal_mode(&reopened).await.is_ok());
        reopened.close().await;
        std::fs::remove_dir_all(base).unwrap();
    }
}
