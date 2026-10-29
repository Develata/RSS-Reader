//! A small file system for the real disk-full cases (phase A completion gate). The variable
//! `RSSR_DISK_FULL_DIR` names a directory on one; without it the cases say so and pass without
//! running. On Linux an unprivileged tmpfs serves, see `docs/testing/native-disk-full.md`.
//!
//! The cases fill the whole file system, so each test binary has at most one of them.

use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

/// A fresh directory for `label` on the small file system, or `None` when none is configured.
pub fn dir(label: &str) -> Option<PathBuf> {
    let Some(root) = std::env::var_os("RSSR_DISK_FULL_DIR") else {
        eprintln!("skipped: RSSR_DISK_FULL_DIR is not set (see docs/testing/native-disk-full.md)");
        return None;
    };
    let dir = PathBuf::from(root).join(label);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    Some(dir)
}

/// Fills the file system with a file in `dir`, then gives back about `headroom` bytes; returns
/// the file, whose removal frees the space again.
pub fn fill(dir: &Path, headroom: u64) -> PathBuf {
    let path = dir.join("filler");
    let mut file = std::fs::File::create(&path).unwrap();
    let chunk = vec![0u8; 64 * 1024];
    let mut written = 0u64;
    loop {
        match file.write(&chunk) {
            Ok(0) => break,
            Ok(count) => written += count as u64,
            Err(error) if error.kind() == ErrorKind::StorageFull => break,
            Err(error) => panic!("filling the file system: {error}"),
        }
    }
    file.set_len(written.saturating_sub(headroom)).unwrap();
    file.sync_all().unwrap();
    path
}
