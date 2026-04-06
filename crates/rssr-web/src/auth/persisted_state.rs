use std::{
    env, fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

const DEFAULT_AUTH_STATE_FILE_NAME: &str = ".rssr-web-auth.json";

#[derive(Serialize, Deserialize)]
pub(super) struct PersistedAuthState {
    pub(super) password_hash: String,
    #[serde(default)]
    pub(super) session_secret: Option<String>,
}

pub(super) fn resolve_auth_state_file() -> PathBuf {
    env::var("RSS_READER_WEB_AUTH_STATE_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| default_auth_state_dir().join(DEFAULT_AUTH_STATE_FILE_NAME))
}

pub(super) fn load_persisted_auth_state(
    auth_state_file: &Path,
) -> Result<Option<PersistedAuthState>> {
    if !auth_state_file.exists() {
        return Ok(None);
    }

    enforce_auth_state_file_permissions(auth_state_file)?;

    let raw = fs::read_to_string(auth_state_file)
        .with_context(|| format!("读取认证状态文件失败：{}", auth_state_file.display()))?;
    let state: PersistedAuthState = serde_json::from_str(&raw)
        .with_context(|| format!("解析认证状态文件失败：{}", auth_state_file.display()))?;
    Ok(Some(state))
}

pub(super) fn persist_auth_state(auth_state_file: &Path, state: &PersistedAuthState) -> Result<()> {
    ensure!(!state.password_hash.trim().is_empty(), "写入认证状态文件失败：缺少密码哈希");
    if let Some(session_secret) = state.session_secret.as_deref() {
        ensure!(session_secret.len() >= 32, "RSS_READER_WEB_SESSION_SECRET 至少需要 32 个字符");
    }

    if let Some(parent) = auth_state_file.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("创建认证状态目录失败：{}", parent.display()))?;
    }

    let payload = serde_json::to_string_pretty(state).context("序列化认证状态文件失败")?;
    write_auth_state_file(auth_state_file, &payload)
}

fn default_auth_state_dir() -> PathBuf {
    path_from_env("HOME")
        .or_else(|| path_from_env("USERPROFILE"))
        .or_else(home_drive_home_path)
        .or_else(|| env::current_dir().ok())
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| PathBuf::from("."))
}

fn path_from_env(name: &str) -> Option<PathBuf> {
    env::var_os(name).map(PathBuf::from).filter(|path| !path.as_os_str().is_empty())
}

fn home_drive_home_path() -> Option<PathBuf> {
    let drive = env::var_os("HOMEDRIVE")?;
    let path = env::var_os("HOMEPATH")?;
    let mut base = PathBuf::from(drive);
    base.push(path);
    (!base.as_os_str().is_empty()).then_some(base)
}

fn enforce_auth_state_file_permissions(auth_state_file: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        fs::set_permissions(auth_state_file, fs::Permissions::from_mode(0o600))
            .with_context(|| format!("收紧认证状态文件权限失败：{}", auth_state_file.display()))?;
    }

    #[cfg(not(unix))]
    {
        let _ = auth_state_file;
    }

    Ok(())
}

fn write_auth_state_file(auth_state_file: &Path, payload: &str) -> Result<()> {
    #[cfg(unix)]
    {
        use std::fs::OpenOptions;
        use std::io::Write;
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .mode(0o600)
            .open(auth_state_file)
            .with_context(|| format!("写入认证状态文件失败：{}", auth_state_file.display()))?;
        file.write_all(payload.as_bytes())
            .with_context(|| format!("写入认证状态文件失败：{}", auth_state_file.display()))?;
        file.sync_all()
            .with_context(|| format!("同步认证状态文件失败：{}", auth_state_file.display()))?;
        fs::set_permissions(auth_state_file, fs::Permissions::from_mode(0o600))
            .with_context(|| format!("收紧认证状态文件权限失败：{}", auth_state_file.display()))?;
        Ok(())
    }

    #[cfg(not(unix))]
    {
        fs::write(auth_state_file, payload)
            .with_context(|| format!("写入认证状态文件失败：{}", auth_state_file.display()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::resolve_auth_state_file;

    unsafe fn set_test_var(name: &str, value: &str) {
        unsafe {
            std::env::set_var(name, value);
        }
    }

    unsafe fn remove_test_var(name: &str) {
        unsafe {
            std::env::remove_var(name);
        }
    }

    #[test]
    fn auth_state_file_prefers_userprofile_when_home_is_missing() {
        unsafe {
            remove_test_var("RSS_READER_WEB_AUTH_STATE_FILE");
            remove_test_var("HOME");
            remove_test_var("HOMEDRIVE");
            remove_test_var("HOMEPATH");
            set_test_var("USERPROFILE", r"C:\Users\rssr");
        }

        let resolved = resolve_auth_state_file();

        unsafe {
            remove_test_var("USERPROFILE");
        }

        assert_eq!(resolved, std::path::Path::new(r"C:\Users\rssr\.rssr-web-auth.json"));
    }
}
