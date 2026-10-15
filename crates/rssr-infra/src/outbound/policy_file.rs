//! The outbound policy of a data directory: `outbound.json` (v0.2 phase C, C1, decision Q3).
//!
//! The file makes the server and every CLI run on the same data directory public-only. Its absence
//! is security-relevant (absent means unrestricted), so only a confirmed absence counts: a symbolic
//! link (dangling or not), anything other than a regular file, an unreadable or malformed file,
//! and, on Unix, a file or directory not owned by the user running the program or writable by
//! others are errors, raised before any request is made.
//!
//! ```json
//! { "mode": "public_only", "allow": ["10.1.0.0/16", "dav.internal"], "deny": ["2001:db8:64::/96"] }
//! ```
//!
//! `allow` holds networks (CIDR or single addresses) and host names; `deny` holds networks.

use std::io::{ErrorKind, Read, Write};
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use super::{Network, OutboundPolicy, PublicOnly};

/// The policy file's name in a data directory.
pub const POLICY_FILE: &str = "outbound.json";

const DEFAULT_POLICY: &str =
    "{\n  \"mode\": \"public_only\",\n  \"allow\": [],\n  \"deny\": []\n}\n";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PolicyFile {
    mode: Mode,
    #[serde(default)]
    allow: Vec<String>,
    #[serde(default)]
    deny: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Mode {
    PublicOnly,
}

/// The policy of `data_dir`: unrestricted when the directory or its policy file does not exist,
/// public-only as the file says otherwise; any other state is an error.
pub fn load_policy(data_dir: &Path) -> Result<OutboundPolicy> {
    match std::fs::metadata(data_dir) {
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return Ok(OutboundPolicy::Unrestricted);
        }
        Err(error) => {
            return Err(error).with_context(|| format!("无法读取数据目录 {}", data_dir.display()));
        }
        Ok(metadata) if !metadata.is_dir() => bail!("数据目录 {} 不是目录", data_dir.display()),
        Ok(_) => {}
    }
    let path = data_dir.join(POLICY_FILE);
    let Some(mut file) = open_without_following(&path)? else {
        return Ok(OutboundPolicy::Unrestricted);
    };
    let metadata = file.metadata().with_context(|| format!("无法读取 {}", path.display()))?;
    if !metadata.is_file() {
        bail!("出站策略 {} 不是普通文件", path.display());
    }
    #[cfg(unix)]
    check_ownership(data_dir, &path, &metadata)?;
    let mut text = String::new();
    file.read_to_string(&mut text).with_context(|| format!("无法读取 {}", path.display()))?;
    parse(&text).with_context(|| format!("出站策略 {} 无效", path.display()))
}

/// At server start, before any outbound work: publishes the default public-only policy when the
/// data directory has none, then loads it.
///
/// The file appears complete or not at all: the contents are written and synced under a temporary
/// name, then linked to the policy name, which fails rather than replace a policy an administrator
/// (or another starting server) put there meanwhile; that one is loaded instead. A crash leaves at
/// most a temporary `outbound.json.new-*` file, never read.
pub fn ensure_server_policy(data_dir: &Path) -> Result<OutboundPolicy> {
    let path = data_dir.join(POLICY_FILE);
    let absent = match std::fs::symlink_metadata(&path) {
        Err(error) if error.kind() == ErrorKind::NotFound => true,
        Err(error) => return Err(error).with_context(|| format!("无法读取 {}", path.display())),
        Ok(_) => false,
    };
    if absent {
        publish_default(data_dir, &path)?;
    }
    match load_policy(data_dir)? {
        OutboundPolicy::Unrestricted => bail!("出站策略 {} 写入后仍不存在", path.display()),
        policy => Ok(policy),
    }
}

fn publish_default(data_dir: &Path, path: &Path) -> Result<()> {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    let temporary = data_dir.join(format!("{POLICY_FILE}.new-{}-{nonce}", std::process::id()));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let written = options.open(&temporary).and_then(|mut file| {
        file.write_all(DEFAULT_POLICY.as_bytes())?;
        file.sync_all()
    });
    let published = written.and_then(|()| match std::fs::hard_link(&temporary, path) {
        Err(error) if error.kind() == ErrorKind::AlreadyExists => Ok(()),
        linked => linked,
    });
    let _ = std::fs::remove_file(&temporary);
    published.with_context(|| format!("无法写入默认出站策略 {}", path.display()))?;
    #[cfg(unix)]
    std::fs::File::open(data_dir)
        .and_then(|directory| directory.sync_all())
        .with_context(|| format!("无法同步数据目录 {}", data_dir.display()))?;
    Ok(())
}

fn parse(text: &str) -> Result<OutboundPolicy> {
    let file: PolicyFile = serde_json::from_str(text)?;
    let Mode::PublicOnly = file.mode;
    let mut hosts = Vec::new();
    let mut networks = Vec::new();
    for entry in file.allow {
        match entry.parse::<Network>() {
            Ok(network) => networks.push(network),
            Err(_) if is_host_name(&entry) => hosts.push(entry),
            Err(_) => bail!("allow 中的条目既不是地址或网段，也不是主机名：{entry:?}"),
        }
    }
    let deny = file
        .deny
        .iter()
        .map(|entry| entry.parse::<Network>().map_err(anyhow::Error::msg))
        .collect::<Result<_>>()?;
    Ok(OutboundPolicy::PublicOnly(PublicOnly::new(hosts, networks, deny)))
}

/// A DNS host name: labels of letters, digits and hyphens, at most one trailing dot.
fn is_host_name(text: &str) -> bool {
    let name = text.strip_suffix('.').unwrap_or(text);
    !name.is_empty()
        && name.len() <= 253
        && name.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
        })
}

/// Opens `path` for reading without following a symbolic link; `None` only when nothing is there.
fn open_without_following(path: &Path) -> Result<Option<std::fs::File>> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    // Not following a link, and not blocking on a FIFO put in its place: the type, owner and mode
    // are then checked on this descriptor, and the policy is read through it.
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::custom_flags(
        &mut options,
        libc::O_NOFOLLOW | libc::O_NONBLOCK,
    );
    #[cfg(not(unix))]
    match std::fs::symlink_metadata(path) {
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).with_context(|| format!("无法读取 {}", path.display())),
        Ok(metadata) if metadata.file_type().is_symlink() => {
            bail!("出站策略 {} 是符号链接", path.display())
        }
        Ok(_) => {}
    }
    match options.open(path) {
        Ok(file) => Ok(Some(file)),
        // With O_NOFOLLOW a symbolic link (dangling or not) fails with ELOOP, so NotFound means the
        // entry is absent (the directory itself was checked first).
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("无法打开出站策略 {}", path.display())),
    }
}

/// The policy is the server user's: the directory and the file belong to the user running this
/// program, and the file is not writable by group or others.
#[cfg(unix)]
fn check_ownership(data_dir: &Path, path: &Path, metadata: &std::fs::Metadata) -> Result<()> {
    use std::os::unix::fs::MetadataExt;
    // SAFETY: geteuid has no preconditions and cannot fail.
    let user = unsafe { libc::geteuid() };
    let dir_owner = std::fs::metadata(data_dir)?.uid();
    if dir_owner != user || metadata.uid() != user {
        bail!(
            "数据目录 {} 属于用户 {dir_owner}，出站策略属于用户 {}，当前用户是 {user}：请以数据目录的所有者运行",
            data_dir.display(),
            metadata.uid()
        );
    }
    if metadata.mode() & 0o022 != 0 {
        bail!("出站策略 {} 可被组或其他用户写入", path.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{OutboundPolicy, POLICY_FILE, ensure_server_policy, load_policy};

    fn temp_dir(label: &str) -> PathBuf {
        let nonce =
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("rssr-outbound-{label}-{nonce}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write(dir: &Path, text: &str) {
        std::fs::write(dir.join(POLICY_FILE), text).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(dir.join(POLICY_FILE), std::fs::Permissions::from_mode(0o600))
                .unwrap();
        }
    }

    #[test]
    fn absence_is_unrestricted_and_a_file_is_public_only() {
        let dir = temp_dir("load");
        assert_eq!(load_policy(&dir).unwrap(), OutboundPolicy::Unrestricted);
        assert_eq!(load_policy(&dir.join("missing")).unwrap(), OutboundPolicy::Unrestricted);

        write(
            &dir,
            r#"{"mode":"public_only","allow":["10.1.0.0/16","DAV.internal.","fd00::1"],"deny":["10.1.2.0/24"]}"#,
        );
        let OutboundPolicy::PublicOnly(policy) = load_policy(&dir).unwrap() else {
            panic!("public-only")
        };
        let ip = |raw: &str| raw.parse().unwrap();
        assert!(policy.allows(None, ip("10.1.9.9")));
        assert!(!policy.allows(None, ip("10.1.2.3")));
        assert!(policy.allows(Some("dav.internal"), ip("192.168.0.2")));
        assert!(policy.allows(None, ip("fd00::1")));
        assert!(!policy.allows(None, ip("127.0.0.1")));
        std::fs::remove_dir_all(dir).unwrap();
    }

    /// Nothing but a well-formed regular file is a policy; every other state fails closed.
    #[test]
    fn anything_but_a_valid_regular_file_is_an_error() {
        let dir = temp_dir("invalid");
        for text in [
            "",
            "{}",
            r#"{"mode":"unrestricted"}"#,
            r#"{"mode":"public_only","extra":1}"#,
            r#"{"mode":"public_only","allow":["not a host!"]}"#,
            r#"{"mode":"public_only","deny":["dav.internal"]}"#,
            r#"{"mode":"public_only","allow":["10.0.0.0/33"]}"#,
        ] {
            write(&dir, text);
            assert!(load_policy(&dir).is_err(), "{text:?}");
        }
        std::fs::remove_file(dir.join(POLICY_FILE)).unwrap();
        std::fs::create_dir(dir.join(POLICY_FILE)).unwrap();
        assert!(load_policy(&dir).is_err(), "a directory");
        std::fs::remove_dir(dir.join(POLICY_FILE)).unwrap();
        let file = dir.join("not-a-directory");
        std::fs::write(&file, "").unwrap();
        assert!(load_policy(&file).is_err(), "a data directory that is a file");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn links_and_writable_files_are_errors() {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp_dir("unix");
        std::os::unix::fs::symlink(dir.join("nowhere.json"), dir.join(POLICY_FILE)).unwrap();
        assert!(load_policy(&dir).is_err(), "a dangling link is not an absence");
        std::fs::remove_file(dir.join(POLICY_FILE)).unwrap();
        std::fs::write(dir.join("real.json"), r#"{"mode":"public_only"}"#).unwrap();
        std::os::unix::fs::symlink(dir.join("real.json"), dir.join(POLICY_FILE)).unwrap();
        assert!(load_policy(&dir).is_err(), "a link to a valid file");
        std::fs::remove_file(dir.join(POLICY_FILE)).unwrap();
        write(&dir, r#"{"mode":"public_only"}"#);
        std::fs::set_permissions(dir.join(POLICY_FILE), std::fs::Permissions::from_mode(0o620))
            .unwrap();
        assert!(load_policy(&dir).is_err(), "group-writable");
        std::fs::remove_dir_all(dir).unwrap();
    }

    /// The server writes the default once and never replaces a policy that is already there.
    #[test]
    fn the_server_default_is_written_only_when_absent() {
        let dir = temp_dir("server");
        let OutboundPolicy::PublicOnly(policy) = ensure_server_policy(&dir).unwrap() else {
            panic!("public-only")
        };
        assert!(!policy.allows(None, "10.0.0.1".parse().unwrap()));

        write(&dir, r#"{"mode":"public_only","allow":["10.0.0.0/8"]}"#);
        let OutboundPolicy::PublicOnly(policy) = ensure_server_policy(&dir).unwrap() else {
            panic!("public-only")
        };
        assert!(policy.allows(None, "10.0.0.1".parse().unwrap()), "the administrator's file stays");
        let leftovers = std::fs::read_dir(&dir).unwrap().count();
        assert_eq!(leftovers, 1, "only the policy file, no temporary one");
        std::fs::remove_dir_all(dir).unwrap();
    }

    /// A FIFO in the policy's place is refused at once instead of blocking the open.
    #[cfg(unix)]
    #[test]
    fn a_fifo_is_refused_without_blocking() {
        let dir = temp_dir("fifo");
        let path = std::ffi::CString::new(dir.join(POLICY_FILE).to_str().unwrap()).unwrap();
        // SAFETY: a valid, NUL-terminated path.
        assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
        assert!(load_policy(&dir).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
