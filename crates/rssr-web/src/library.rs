//! 服务端模式的文章库：在数据目录上打开与桌面相同的原生宿主（SQLite 索引库 + 正文库）。
//!
//! 文章库与登录凭据分开存放：凭据由 `auth` 管，路径是 `RSS_READER_WEB_AUTH_STATE_FILE`；
//! 文章库在 `RSS_READER_WEB_DATA_DIR`，有意不与桌面版的 `rss-reader/` 共用，
//! 避免两个进程写同一个库。
use std::{env, path::PathBuf};

use anyhow::{Context, Result, bail};
use rssr_host::{
    HostServices,
    native::{NativeHost, NativeHostOptions},
};
use rssr_infra::{db::sqlite_native::NativeSqliteBackend, outbound::OutboundPolicy};

const DATA_DIR_NAME: &str = "rss-reader-web";
const INDEX_DATABASE_FILE: &str = "rss-reader.db";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LibraryConfig {
    pub(crate) data_dir: PathBuf,
    pub(crate) outbound: OutboundPolicy,
}

pub(crate) fn load_library_config() -> Result<LibraryConfig> {
    let data_dir = match env::var_os("RSS_READER_WEB_DATA_DIR").filter(|value| !value.is_empty()) {
        Some(dir) => PathBuf::from(dir),
        None => default_data_dir()?,
    };
    // 服务端默认只访问公网地址；局域网 feed 需要显式放开（见 docs/deployment/web.md）。
    let outbound = if env_flag("RSS_READER_WEB_ALLOW_PRIVATE_FEEDS") {
        OutboundPolicy::Unrestricted
    } else {
        OutboundPolicy::PublicOnly
    };
    Ok(LibraryConfig { data_dir, outbound })
}

fn default_data_dir() -> Result<PathBuf> {
    if let Some(dir) = env::var_os("XDG_DATA_HOME").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(dir).join(DATA_DIR_NAME));
    }
    if let Some(home) = env::var_os("HOME").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(home).join(".local").join("share").join(DATA_DIR_NAME));
    }
    bail!("无法确定文章库目录：请设置 RSS_READER_WEB_DATA_DIR")
}

fn env_flag(name: &str) -> bool {
    env::var(name).is_ok_and(|raw| matches!(raw.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
}

/// 打开文章库并启动自动刷新。自动刷新不依赖任何浏览器在线。
///
/// 一个进程只调用一次：`NativeHost` 的后台任务活到进程结束（见其文档）。
pub(crate) async fn open_library(config: &LibraryConfig) -> Result<HostServices> {
    std::fs::create_dir_all(&config.data_dir)
        .with_context(|| format!("创建文章库目录失败：{}", config.data_dir.display()))?;
    let backend = NativeSqliteBackend::with_path(config.data_dir.join(INDEX_DATABASE_FILE));
    let host =
        NativeHost::open_with_options(backend, NativeHostOptions { outbound: config.outbound })
            .await
            .context("打开文章库失败")?;
    let services = HostServices::new(host.use_cases(), host.host_capabilities());
    services.ensure_auto_refresh_started();
    Ok(services)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use rssr_infra::outbound::OutboundPolicy;

    use super::load_library_config;
    use crate::auth::test_env;

    unsafe fn set(name: &str, value: &str) {
        unsafe { std::env::set_var(name, value) }
    }

    unsafe fn remove(name: &str) {
        unsafe { std::env::remove_var(name) }
    }

    #[test]
    fn data_dir_and_outbound_policy_come_from_the_environment() {
        const VARS: [&str; 3] =
            ["RSS_READER_WEB_DATA_DIR", "RSS_READER_WEB_ALLOW_PRIVATE_FEEDS", "XDG_DATA_HOME"];
        let _guard = test_env::lock();
        let saved = VARS.map(std::env::var_os);
        unsafe {
            set("RSS_READER_WEB_DATA_DIR", "/srv/rssr");
            remove("RSS_READER_WEB_ALLOW_PRIVATE_FEEDS");
        }
        let config = load_library_config().expect("config");
        assert_eq!(config.data_dir, PathBuf::from("/srv/rssr"));
        assert_eq!(config.outbound, OutboundPolicy::PublicOnly);

        unsafe {
            set("RSS_READER_WEB_ALLOW_PRIVATE_FEEDS", "true");
            remove("RSS_READER_WEB_DATA_DIR");
            set("XDG_DATA_HOME", "/xdg");
        }
        let config = load_library_config().expect("config");
        assert_eq!(config.data_dir, PathBuf::from("/xdg").join("rss-reader-web"));
        assert_eq!(config.outbound, OutboundPolicy::Unrestricted);

        for (name, value) in VARS.into_iter().zip(saved) {
            match value {
                Some(value) => unsafe { std::env::set_var(name, value) },
                None => unsafe { remove(name) },
            }
        }
    }
}
