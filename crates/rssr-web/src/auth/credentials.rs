//! 所有者凭据：Argon2 哈希，以及认证库建立时的初始凭据（决定 Q1）。

use std::env;

use anyhow::{Result, anyhow, bail, ensure};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier, password_hash::SaltString};
use rand_core::OsRng;

use super::legacy_state::{read_password_hash, resolve_auth_state_file};

/// 设置了也只在认证库建立时读取的凭据变量。
pub(crate) const SEED_VARIABLES: [&str; 2] =
    ["RSS_READER_WEB_PASSWORD_HASH", "RSS_READER_WEB_PASSWORD"];

pub(crate) fn generate_password_hash(password: &str) -> Result<String> {
    ensure!(password.len() >= 8, "密码至少需要 8 个字符");
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|_| anyhow!("生成密码哈希失败"))?;
    Ok(hash.to_string())
}

pub(crate) fn verify_password_hash(hash: &str, password: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(hash) else {
        return false;
    };
    Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok()
}

/// 用户名不对也照样做一次 Argon2 校验：否则错误用户名立即返回、正确用户名才耗时，
/// 响应时间就泄露了哪个用户名是真的（审计 F2）。
pub(crate) fn credentials_match(
    password_hash: &str,
    expected_username: &str,
    username: &str,
    password: &str,
) -> bool {
    let password_matches = verify_password_hash(password_hash, password);
    password_matches && username.trim() == expected_username
}

/// 认证库还没有凭据时的初始凭据：按顺序取第一个可用的，返回哈希与它的来源。
///
/// 1. `RSS_READER_WEB_PASSWORD_HASH`；
/// 2. `RSS_READER_WEB_PASSWORD`（在这里生成哈希）；
/// 3. v0.1 认证状态文件里的哈希（只读）。
pub(crate) fn initial_credential() -> Result<(String, String)> {
    if let Some(hash) = optional_env("RSS_READER_WEB_PASSWORD_HASH") {
        validate_password_hash(&hash, "RSS_READER_WEB_PASSWORD_HASH")?;
        return Ok((hash, "RSS_READER_WEB_PASSWORD_HASH".to_owned()));
    }
    if let Some(password) = optional_env("RSS_READER_WEB_PASSWORD") {
        return Ok((generate_password_hash(&password)?, "RSS_READER_WEB_PASSWORD".to_owned()));
    }
    let file = resolve_auth_state_file();
    if let Some(hash) = read_password_hash(&file)? {
        let source = format!("旧认证状态文件 {}", file.display());
        validate_password_hash(&hash, &source)?;
        return Ok((hash, source));
    }
    bail!(
        "认证库还没有凭据，也没有初始凭据：请设置 RSS_READER_WEB_PASSWORD_HASH 或 RSS_READER_WEB_PASSWORD，或者运行 rssr-web passwd"
    )
}

fn optional_env(name: &str) -> Option<String> {
    env::var(name).ok().and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

fn validate_password_hash(raw: &str, source: &str) -> Result<()> {
    PasswordHash::new(raw).map_err(|_| anyhow!("{source} 不是合法的 Argon2 哈希"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::test_env;

    unsafe fn set(name: &str, value: impl AsRef<std::ffi::OsStr>) {
        unsafe { env::set_var(name, value) }
    }

    unsafe fn clear() {
        for name in [SEED_VARIABLES.as_slice(), &["RSS_READER_WEB_AUTH_STATE_FILE"]].concat() {
            unsafe { env::remove_var(name) }
        }
    }

    #[test]
    fn generated_argon2_hash_verifies() {
        let hash = generate_password_hash("adminadmin").expect("hash");
        assert!(verify_password_hash(&hash, "adminadmin"));
        assert!(!verify_password_hash(&hash, "wrong-password"));
        assert!(generate_password_hash("short").is_err());
    }

    /// 顺序：哈希变量、明文变量、旧认证状态文件；都没有时报错。
    #[test]
    fn the_initial_credential_comes_from_the_first_available_source() {
        let _guard = test_env::lock();
        let dir = crate::auth::test_support::ScratchDir::new("seed");
        let legacy = dir.path().join("auth.json");
        let legacy_hash = generate_password_hash("from-the-file").unwrap();
        std::fs::write(&legacy, format!(r#"{{"password_hash":"{legacy_hash}"}}"#)).unwrap();
        let env_hash = generate_password_hash("from-the-hash").unwrap();

        unsafe {
            clear();
            set("RSS_READER_WEB_AUTH_STATE_FILE", &legacy);
            set("RSS_READER_WEB_PASSWORD", "from-the-password");
            set("RSS_READER_WEB_PASSWORD_HASH", &env_hash);
        }
        assert_eq!(initial_credential().unwrap(), (env_hash, SEED_VARIABLES[0].to_owned()));

        unsafe { env::remove_var("RSS_READER_WEB_PASSWORD_HASH") }
        let (hash, source) = initial_credential().unwrap();
        assert!(verify_password_hash(&hash, "from-the-password"));
        assert_eq!(source, SEED_VARIABLES[1]);

        unsafe { env::remove_var("RSS_READER_WEB_PASSWORD") }
        let (hash, source) = initial_credential().unwrap();
        assert_eq!(hash, legacy_hash);
        assert!(source.contains("auth.json"), "{source}");

        unsafe { set("RSS_READER_WEB_AUTH_STATE_FILE", dir.path().join("missing.json")) }
        assert!(initial_credential().is_err());
        unsafe { set("RSS_READER_WEB_PASSWORD_HASH", "not-a-hash") }
        assert!(initial_credential().is_err(), "an invalid hash is not skipped");
        unsafe { clear() }
    }
}
