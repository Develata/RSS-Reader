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
///
/// 设置了却不可用的来源（不是 Unicode、不是可用的 Argon2 哈希）是错误，不跳到下一个来源。
pub(crate) fn initial_credential() -> Result<(String, String)> {
    if let Some(hash) = optional_env("RSS_READER_WEB_PASSWORD_HASH")? {
        validate_password_hash(&hash, "RSS_READER_WEB_PASSWORD_HASH")?;
        return Ok((hash, "RSS_READER_WEB_PASSWORD_HASH".to_owned()));
    }
    if let Some(password) = optional_env("RSS_READER_WEB_PASSWORD")? {
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

/// 未设置或为空（Compose 用空字符串表示未提供）时为 `None`；不是 Unicode 时是错误。
fn optional_env(name: &str) -> Result<Option<String>> {
    match env::var(name) {
        Ok(value) => {
            let trimmed = value.trim();
            Ok((!trimmed.is_empty()).then(|| trimmed.to_string()))
        }
        Err(env::VarError::NotPresent) => Ok(None),
        Err(env::VarError::NotUnicode(_)) => bail!("环境变量 {name} 不是合法的 Unicode"),
    }
}

/// 只接受能核对密码的 Argon2 哈希：算法、版本与参数受支持，并且带盐与哈希值。通用的 PHC
/// 解析允许省略盐与哈希值（例如 `$argon2id`），这样的串核对不了任何密码。不靠试算一次来
/// 判断：缺少盐或哈希值时核对结果与密码错误无法区分，试算还可能按参数分配大量内存。
fn validate_password_hash(raw: &str, source: &str) -> Result<()> {
    let usable = PasswordHash::new(raw).is_ok_and(|hash| {
        let mut salt = [0_u8; 64];
        argon2::Algorithm::try_from(hash.algorithm).is_ok()
            && hash.version.is_none_or(|version| argon2::Version::try_from(version).is_ok())
            && argon2::Params::try_from(&hash).is_ok()
            && hash.hash.is_some()
            && hash.salt.is_some_and(|salt_b64| {
                salt_b64
                    .decode_b64(&mut salt)
                    .is_ok_and(|bytes| bytes.len() >= argon2::MIN_SALT_LEN)
            })
    });
    ensure!(
        usable,
        "{source} 不是可用的 Argon2 哈希：需要 argon2id、argon2i 或 argon2d，带合法的参数、盐与哈希值"
    );
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

    /// 只有能核对密码的 Argon2 哈希才能成为初始凭据（评审 S2）。
    #[test]
    fn only_usable_argon2_hashes_are_accepted() {
        let generated = generate_password_hash("a-usable-password").unwrap();
        validate_password_hash(&generated, "generated").unwrap();
        for unusable in [
            "$argon2id",
            "$argon2id$v=19$m=19456,t=2,p=1",
            "$argon2id$v=19$m=19456,t=2,p=1$c2FsdHNhbHQ",
            "$argon2id$v=19$m=19456,t=2,p=1$c2FsdA$aGFzaGhhc2hoYXNoaGFzaA",
            "$argon2id$v=99$m=19456,t=2,p=1$c2FsdHNhbHQ$aGFzaGhhc2hoYXNoaGFzaA",
            "$argon2id$v=19$m=1,t=2,p=1$c2FsdHNhbHQ$aGFzaGhhc2hoYXNoaGFzaA",
            "$argon2id$v=19$m=19456,t=2,p=1,x=1$c2FsdHNhbHQ$aGFzaGhhc2hoYXNoaGFzaA",
            "$scrypt$ln=16,r=8,p=1$c2FsdHNhbHQ$aGFzaGhhc2hoYXNoaGFzaA",
            "not-a-hash",
        ] {
            assert!(validate_password_hash(unusable, "test").is_err(), "{unusable}");
        }
        validate_password_hash(
            "$argon2id$v=19$m=19456,t=2,p=1$c2FsdHNhbHQ$aGFzaGhhc2hoYXNoaGFzaA",
            "well-formed",
        )
        .expect("a well-formed hash");
    }

    /// 设置了却不是 Unicode 的凭据变量是错误，不当作未设置而换用下一个来源（评审 S3）。
    #[cfg(unix)]
    #[test]
    fn a_seed_variable_that_is_not_unicode_is_an_error() {
        use std::os::unix::ffi::OsStrExt;
        let _guard = test_env::lock();
        unsafe {
            clear();
            set("RSS_READER_WEB_PASSWORD_HASH", std::ffi::OsStr::from_bytes(b"\xff\xfe"));
            set("RSS_READER_WEB_PASSWORD", "a-fallback-password");
        }
        let error = initial_credential().unwrap_err().to_string();
        assert!(error.contains("RSS_READER_WEB_PASSWORD_HASH"), "{error}");
        unsafe { clear() }
    }
}
