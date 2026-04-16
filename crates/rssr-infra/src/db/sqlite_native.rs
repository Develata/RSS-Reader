use async_trait::async_trait;
use std::path::{Path, PathBuf};

use anyhow::Context;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

use crate::db::{
    SqlitePool, create_sqlite_pool, default_sqlite_max_connections, migrate, migrate_content,
    storage_backend::StorageBackend,
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
        Ok(Self::with_path(default_database_path()?))
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

fn default_database_path() -> anyhow::Result<PathBuf> {
    #[cfg(target_os = "android")]
    {
        let base_dir = android_data_base_dir()?;
        return Ok(database_path_in_base_dir(&base_dir));
    }

    #[cfg(not(target_os = "android"))]
    {
        let executable_path = std::env::current_exe().context("无法定位可执行文件路径")?;
        let base_dir = executable_path.parent().context("无法定位可执行文件所在目录")?;

        Ok(database_path_in_base_dir(base_dir))
    }
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

fn database_path_in_base_dir(base_dir: &Path) -> PathBuf {
    base_dir.join("RSS-Reader").join("rss-reader.db")
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

    let options = SqliteConnectOptions::new().filename(database_path).create_if_missing(true);

    SqlitePoolOptions::new()
        .max_connections(default_sqlite_max_connections(database_path.to_string_lossy().as_ref()))
        .connect_with(options)
        .await
        .with_context(|| format!("打开本地数据库失败: {}", database_path.display()))
}

fn derive_content_database_url(database_url: &str) -> anyhow::Result<String> {
    if database_url == "sqlite::memory:" || database_url.contains("mode=memory") {
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
        NativeSqliteBackend, append_content_suffix, content_database_path,
        database_path_in_base_dir,
    };
    use crate::db::storage_backend::StorageBackend;
    use std::{
        path::Path,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn database_path_uses_project_subdirectory() {
        let path = database_path_in_base_dir(Path::new("/tmp/example"));
        assert_eq!(path, Path::new("/tmp/example/RSS-Reader/rss-reader.db"));
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
}
