//! `rssr-cli backup` as a process (phase C5): a backup killed while it copies leaves only its
//! scratch file, which the next backup of that target removes before it writes a complete copy;
//! two processes backing up one target publish once; a data directory without a library is
//! refused and left without one.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use rssr_infra::store::LibraryStore;

struct Dir(PathBuf);

impl Dir {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).expect("clock").as_nanos();
        let path = std::env::temp_dir()
            .join(format!("rssr-cli-backup-{label} 中文-{}-{nonce}", std::process::id()));
        std::fs::create_dir_all(path.join("data")).expect("temp dir");
        Self(path)
    }

    fn data(&self) -> PathBuf {
        self.0.join("data")
    }

    fn scratch_names(&self) -> Vec<String> {
        std::fs::read_dir(&self.0)
            .expect("read dir")
            .map(|entry| entry.expect("entry").file_name().to_string_lossy().into_owned())
            .filter(|name| name.contains(".partial-"))
            .collect()
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn cli(data: &Path, arguments: &[&std::ffi::OsStr]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rssr-cli"));
    command.arg("--data-dir").arg(data).args(arguments);
    command
}

/// A library large enough that copying it takes a while: about 64 MiB of padding rows.
async fn large_library(data: &Path) {
    let store = LibraryStore::open(data.join("library.sqlite3")).await.expect("open library");
    store
        .write(|tx| {
            Ok(tx.execute_batch(
                "CREATE TABLE padding (bytes BLOB NOT NULL);
                 WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 65536)
                 INSERT INTO padding SELECT randomblob(1024) FROM n;",
            )?)
        })
        .await
        .expect("padding");
}

async fn padding_rows(path: &Path) -> i64 {
    let store = LibraryStore::open(path).await.expect("open backup");
    store
        .read(|connection| {
            Ok(connection.query_row("SELECT count(*) FROM padding", [], |row| row.get(0))?)
        })
        .await
        .expect("count")
}

#[tokio::test(flavor = "multi_thread")]
async fn a_backup_killed_while_copying_is_cleaned_up_by_the_next() {
    let dir = Dir::new("killed");
    large_library(&dir.data()).await;
    let target = dir.0.join("backup.sqlite3");

    let mut child = cli(&dir.data(), &["backup".as_ref(), target.as_os_str()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("start backup");
    let started = Instant::now();
    // Killed once its scratch file itself (not a companion file of SQLite's) is being filled.
    loop {
        let filling = dir.scratch_names().iter().any(|name| {
            !["-journal", "-wal", "-shm"].iter().any(|suffix| name.ends_with(suffix))
                && std::fs::metadata(dir.0.join(name)).is_ok_and(|metadata| metadata.len() > 0)
        });
        if filling {
            break;
        }
        assert!(child.try_wait().expect("poll").is_none(), "the backup ended before it was killed");
        assert!(started.elapsed() < Duration::from_secs(60), "the copy never started");
        std::thread::sleep(Duration::from_millis(5));
    }
    child.kill().expect("kill backup");
    child.wait().expect("reap backup");
    assert!(!target.exists(), "nothing published by a killed copy");
    assert!(!dir.scratch_names().is_empty(), "the killed backup left its scratch file");

    let output =
        cli(&dir.data(), &["backup".as_ref(), target.as_os_str()]).output().expect("backup");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(dir.scratch_names().is_empty(), "{:?}", dir.scratch_names());
    assert_eq!(padding_rows(&target).await, 65_536);
}

/// Two processes back up to one target at once: one publishes; the other waits for the lock,
/// then finds the target taken (or gives up waiting). Nothing else is left (review C5-R2-06).
#[tokio::test(flavor = "multi_thread")]
async fn two_processes_backing_up_one_target_publish_once() {
    let dir = Dir::new("two");
    large_library(&dir.data()).await;
    let target = dir.0.join("backup.sqlite3");
    let start = || {
        cli(&dir.data(), &["backup".as_ref(), target.as_os_str()])
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start backup")
    };
    let (first, second) = (start(), start());
    let outputs =
        [first.wait_with_output().expect("first"), second.wait_with_output().expect("second")];
    let errors: Vec<String> = outputs
        .iter()
        .filter(|output| !output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stderr).into_owned())
        .collect();
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(
        errors[0].contains("已存在") || errors[0].contains("另一个备份仍在写入"),
        "{}",
        errors[0]
    );
    assert!(dir.scratch_names().is_empty(), "{:?}", dir.scratch_names());
    assert_eq!(padding_rows(&target).await, 65_536);
}

#[test]
fn a_data_directory_without_a_library_is_refused_and_left_alone() {
    let dir = Dir::new("empty");
    let target = dir.0.join("backup.sqlite3");
    let output =
        cli(&dir.data(), &["backup".as_ref(), target.as_os_str()]).output().expect("backup");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("没有找到资料库"));
    assert!(!dir.data().join("library.sqlite3").exists());
    assert!(!target.exists());
}
