//! v0.1 的认证状态文件（`RSS_READER_WEB_AUTH_STATE_FILE`，默认 `$HOME/.rssr-web-auth.json`）。
//!
//! 认证库建立时，它里面的密码哈希是第三个初始凭据来源（决定 Q1）。这里只读它，从不写入或改动
//! 它的权限：升级期间旧认证卷可能只读挂载。文件里的其他字段（旧的 session secret）不再使用。

use std::{
    env, fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::Deserialize;

const DEFAULT_AUTH_STATE_FILE_NAME: &str = ".rssr-web-auth.json";

#[derive(Deserialize)]
struct LegacyAuthState {
    password_hash: String,
}

/// 设置了就用它（操作系统路径不必是 Unicode），否则用默认路径。
pub(super) fn resolve_auth_state_file() -> PathBuf {
    env::var_os("RSS_READER_WEB_AUTH_STATE_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| default_auth_state_dir().join(DEFAULT_AUTH_STATE_FILE_NAME))
}

/// 旧文件里的密码哈希；文件不存在时为 `None`。文件存在却读不出或格式不对是错误：
/// 管理员以为会迁移的凭据不能被悄悄跳过。
pub(super) fn read_password_hash(auth_state_file: &Path) -> Result<Option<String>> {
    let raw = match fs::read_to_string(auth_state_file) {
        Ok(raw) => raw,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("读取旧认证状态文件失败：{}", auth_state_file.display()));
        }
    };
    let state: LegacyAuthState = serde_json::from_str(&raw)
        .with_context(|| format!("解析旧认证状态文件失败：{}", auth_state_file.display()))?;
    Ok(Some(state.password_hash))
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

#[cfg(test)]
mod tests {
    use super::{DEFAULT_AUTH_STATE_FILE_NAME, read_password_hash, resolve_auth_state_file};

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
        let _env_guard = crate::auth::test_env::lock();

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

        assert_eq!(
            resolved,
            std::path::PathBuf::from(r"C:\Users\rssr").join(DEFAULT_AUTH_STATE_FILE_NAME)
        );
    }

    /// 读出哈希，不改动文件（内容与权限）；旧的 session secret 被忽略。
    #[test]
    fn the_legacy_file_is_read_without_being_changed() {
        let dir = crate::auth::test_support::ScratchDir::new("legacy-state");
        let file = dir.path().join("auth.json");
        let content = r#"{"password_hash":"$argon2id$v=19$m=19456,t=2,p=1$c2FsdA$aGFzaA","session_secret":"01234567890123456789012345678901"}"#;
        std::fs::write(&file, content).unwrap();
        #[cfg(unix)]
        std::fs::set_permissions(&file, std::os::unix::fs::PermissionsExt::from_mode(0o444))
            .unwrap();
        let before = std::fs::metadata(&file).unwrap().permissions();

        let hash = read_password_hash(&file).unwrap();

        assert_eq!(hash.as_deref(), Some("$argon2id$v=19$m=19456,t=2,p=1$c2FsdA$aGFzaA"));
        assert_eq!(std::fs::read_to_string(&file).unwrap(), content);
        assert_eq!(std::fs::metadata(&file).unwrap().permissions(), before);
        assert_eq!(read_password_hash(&dir.path().join("missing.json")).unwrap(), None);
        let malformed = dir.path().join("malformed.json");
        std::fs::write(&malformed, "not json").unwrap();
        assert!(read_password_hash(&malformed).is_err());
    }
}
