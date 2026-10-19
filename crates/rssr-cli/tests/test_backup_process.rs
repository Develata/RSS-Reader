//! `rssr-cli backup` as a process (phase C5): a backup killed while it copies leaves only its
//! scratch file, which the next backup of that target removes before it writes a complete copy;
//! on Unix, Ctrl-C (SIGINT) while it copies cancels it and leaves nothing; two processes backing
//! up one target publish once; a data directory without a library is refused and left without
//! one.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
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

    /// The scratch directories (`<target>.partial`) and what they hold, as `dir/` and
    /// `dir/entry`. A directory that cannot be read fails the test rather than reading as empty.
    fn scratch_names(&self) -> Vec<String> {
        let mut found = Vec::new();
        for entry in std::fs::read_dir(&self.0).expect("read dir") {
            let name = entry.expect("entry").file_name().to_string_lossy().into_owned();
            if !name.ends_with(".partial") {
                continue;
            }
            found.push(format!("{name}/"));
            for entry in std::fs::read_dir(self.0.join(&name)).expect("read scratch directory") {
                let entry = entry.expect("entry").file_name().to_string_lossy().into_owned();
                found.push(format!("{name}/{entry}"));
            }
        }
        found
    }

    /// Whether a scratch file itself (not a companion file of SQLite's) has content.
    fn scratch_filling(&self) -> bool {
        self.scratch_names().iter().any(|name| {
            !name.ends_with('/')
                && !["-journal", "-wal", "-shm"].iter().any(|suffix| name.ends_with(suffix))
                && std::fs::metadata(self.0.join(name)).is_ok_and(|metadata| metadata.len() > 0)
        })
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Child processes, killed and reaped if the test ends before they do (review C5-R4-05).
struct Running(Vec<Child>);

impl Running {
    /// Waits until the first one's scratch file has content: writing began (whether the copy is
    /// still running when the caller acts is not observed).
    fn until_copying(&mut self, dir: &Dir) {
        let started = Instant::now();
        while !dir.scratch_filling() {
            assert!(
                self.0[0].try_wait().expect("poll").is_none(),
                "the backup ended before its copy was seen"
            );
            assert!(started.elapsed() < Duration::from_secs(60), "the copy never started");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// Waits for all of them, two minutes at most in all, and returns their outputs.
    fn finish(mut self) -> Vec<Output> {
        let deadline = Instant::now() + Duration::from_secs(120);
        while self.0.iter_mut().any(|child| child.try_wait().expect("poll").is_none()) {
            assert!(Instant::now() < deadline, "a backup did not finish within two minutes");
            std::thread::sleep(Duration::from_millis(20));
        }
        std::mem::take(&mut self.0)
            .into_iter()
            .map(|child| child.wait_with_output().expect("output"))
            .collect()
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        for child in &mut self.0 {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn cli(data: &Path, arguments: &[&std::ffi::OsStr]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rssr-cli"));
    command.arg("--data-dir").arg(data).args(arguments);
    command
}

fn start_backup(dir: &Dir, target: &Path) -> Child {
    cli(&dir.data(), &["backup".as_ref(), target.as_os_str()])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start backup")
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

    let mut running = Running(vec![start_backup(&dir, &target)]);
    running.until_copying(&dir);
    running.0[0].kill().expect("kill backup");
    running.finish();
    assert!(!target.exists(), "nothing published by a killed copy");
    assert!(dir.scratch_filling(), "the killed backup left its scratch file");

    let outputs = Running(vec![start_backup(&dir, &target)]).finish();
    assert!(outputs[0].status.success(), "{}", String::from_utf8_lossy(&outputs[0].stderr));
    assert!(dir.scratch_names().is_empty(), "{:?}", dir.scratch_names());
    assert_eq!(padding_rows(&target).await, 65_536);
}

/// Ctrl-C once the copy has begun writing: the command cancels the backup, waits for it to stop
/// and reports it cancelled, with nothing published and nothing left (review C5-R4-04). With a 64
/// MiB copy the signal comes while it runs; that is likely, not observed. Unix only: a test cannot
/// send Ctrl-C to one child process on Windows.
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn ctrl_c_while_copying_cancels_the_backup() {
    let dir = Dir::new("interrupted");
    large_library(&dir.data()).await;
    let target = dir.0.join("backup.sqlite3");

    let mut running = Running(vec![start_backup(&dir, &target)]);
    running.until_copying(&dir);
    let pid = running.0[0].id().to_string();
    let status = Command::new("kill").args(["-INT", &pid]).status().expect("kill -INT");
    assert!(status.success());
    let output = running.finish().remove(0);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{stderr}");
    assert!(stderr.contains("已取消"), "{stderr}");
    assert!(!target.exists());
    assert!(dir.scratch_names().is_empty(), "{:?}", dir.scratch_names());
}

/// Two processes started together to back up one target: one publishes; the other waits for the
/// lock, then finds the target taken (or gives up waiting). That they contend is likely with a 64
/// MiB copy, not observed. Nothing else is left (review C5-R2-06).
#[tokio::test(flavor = "multi_thread")]
async fn two_processes_backing_up_one_target_publish_once() {
    let dir = Dir::new("two");
    large_library(&dir.data()).await;
    let target = dir.0.join("backup.sqlite3");
    // The guard exists before the first child, so a failed second start still reaps the first
    // (review C5-R5-04).
    let mut running = Running(Vec::new());
    running.0.push(start_backup(&dir, &target));
    running.0.push(start_backup(&dir, &target));
    let outputs = running.finish();
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
