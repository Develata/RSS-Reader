use super::*;
use rusqlite::Connection;
use std::sync::atomic::{AtomicU64, Ordering};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!(
            "rssr-android-storage-{}-{stamp}-{} 中文",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn files(&self) -> PathBuf {
        self.0.join("files")
    }
    fn legacy(&self) -> PathBuf {
        self.0.join("cache/RSS-Reader")
    }

    fn seed(&self) {
        fs::create_dir_all(self.legacy()).unwrap();
        fs::write(self.legacy().join("rss-reader.db"), b"old database bytes").unwrap();
        fs::write(self.legacy().join("shell-prefs.json"), br#"{"entry_search":"retained"}"#)
            .unwrap();
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn wal_database(path: &Path, value: &str) -> Connection {
    let conn = Connection::open(path).unwrap();
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0; CREATE TABLE proof(value TEXT NOT NULL);").unwrap();
    conn.execute("INSERT INTO proof VALUES (?1)", [value]).unwrap();
    conn
}

#[test]
fn relocation_keeps_v02_library_v01_pair_wal_and_preferences() {
    let fixture = Fixture::new();
    fs::create_dir_all(fixture.legacy()).unwrap();
    let names = ["library.sqlite3", "rss-reader.db", "rss-reader-content.db"];
    let sources = names.map(|name| wal_database(&fixture.legacy().join(name), name));
    fs::write(fixture.legacy().join("shell-prefs.json"), br#"{"entry_search":"retained"}"#)
        .unwrap();
    let before = manifest(&fixture.legacy()).unwrap();
    let target = relocate(&fixture.files(), &fixture.legacy()).unwrap();
    assert_eq!(manifest(&fixture.legacy()).unwrap(), before);
    assert_eq!(manifest(&target).unwrap(), before);
    for name in names {
        assert!(!target.join(format!("{name}-shm")).exists());
        assert!(fs::metadata(target.join(format!("{name}-wal"))).unwrap().len() > 0);
        let conn = Connection::open(target.join(name)).unwrap();
        let value: String =
            conn.query_row("SELECT value FROM proof", [], |row| row.get(0)).unwrap();
        assert_eq!(value, name);
        let integrity: String =
            conn.query_row("PRAGMA integrity_check", [], |row| row.get(0)).unwrap();
        assert_eq!(integrity, "ok");
        conn.execute("UPDATE proof SET value = 'durable edit'", []).unwrap();
    }
    drop(sources);
    assert_eq!(relocate(&fixture.files(), &fixture.legacy()).unwrap(), target);
    let conn = Connection::open(target.join("library.sqlite3")).unwrap();
    let value: String = conn.query_row("SELECT value FROM proof", [], |row| row.get(0)).unwrap();
    assert_eq!(value, "durable edit", "a later start never overwrites newer data");
}

#[test]
fn interrupted_staging_is_never_used_and_success_does_not_overwrite_later_data() {
    let fixture = Fixture::new();
    fixture.seed();
    let partial = fixture.files().join(".rssr-storage-interrupted.pending");
    fs::create_dir_all(&partial).unwrap();
    fs::write(partial.join("rss-reader.db"), b"incomplete").unwrap();
    let target = relocate(&fixture.files(), &fixture.legacy()).unwrap();
    assert_eq!(fs::read(target.join("rss-reader.db")).unwrap(), b"old database bytes");
    fs::write(target.join("rss-reader.db"), b"new data").unwrap();
    fs::write(fixture.legacy().join("rss-reader.db"), b"stale data from a downgraded app").unwrap();
    assert_eq!(relocate(&fixture.files(), &fixture.legacy()).unwrap(), target);
    assert_eq!(fs::read(target.join("rss-reader.db")).unwrap(), b"new data");
    assert_eq!(fs::read(partial.join("rss-reader.db")).unwrap(), b"incomplete");
}

#[test]
fn unexpected_destination_is_preserved_and_blocks_silent_empty_library() {
    let fixture = Fixture::new();
    fixture.seed();
    let target = local_data_dir_in_base_dir(&fixture.files());
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("rss-reader.db"), b"another library").unwrap();
    let original = manifest(&fixture.legacy()).unwrap();
    assert!(relocate(&fixture.files(), &fixture.legacy()).is_err());
    assert_eq!(manifest(&fixture.legacy()).unwrap(), original);
    assert_eq!(fs::read(target.join("rss-reader.db")).unwrap(), b"another library");
    assert!(!target.join(MARKER).exists());
}

#[test]
fn concurrent_startups_publish_one_complete_directory() {
    let fixture = Fixture::new();
    fixture.seed();
    let handles: Vec<_> = (0..4)
        .map(|_| {
            let files = fixture.files();
            let legacy = fixture.legacy();
            std::thread::spawn(move || relocate(&files, &legacy).unwrap())
        })
        .collect();
    for handle in handles {
        assert_eq!(handle.join().unwrap(), local_data_dir_in_base_dir(&fixture.files()));
    }
    assert!(
        !fs::read_dir(fixture.files()).unwrap().any(|entry| entry
            .unwrap()
            .path()
            .extension()
            .is_some_and(|x| x == "pending"))
    );
}

#[test]
fn first_install_uses_files_without_creating_cache_and_existing_durable_home_is_adopted() {
    let fresh = Fixture::new();
    let target = relocate(&fresh.files(), &fresh.legacy()).unwrap();
    assert!(target.join(MARKER).exists());
    assert!(!fresh.legacy().exists());
    let existing = Fixture::new();
    let durable = local_data_dir_in_base_dir(&existing.files());
    fs::create_dir_all(&durable).unwrap();
    fs::write(durable.join("rss-reader.db"), b"already durable").unwrap();
    assert_eq!(relocate(&existing.files(), &durable).unwrap(), durable);
    assert_eq!(fs::read(durable.join("rss-reader.db")).unwrap(), b"already durable");
}

#[test]
fn orphaned_content_database_cannot_be_hidden_by_creating_a_new_index() {
    let fixture = Fixture::new();
    fs::create_dir_all(fixture.legacy()).unwrap();
    fs::write(fixture.legacy().join("rss-reader-content.db"), b"valuable content").unwrap();
    assert!(relocate(&fixture.files(), &fixture.legacy()).is_err());
    assert!(!local_data_dir_in_base_dir(&fixture.files()).exists());
    assert_eq!(
        fs::read(fixture.legacy().join("rss-reader-content.db")).unwrap(),
        b"valuable content"
    );
}

#[cfg(unix)]
#[test]
fn symlinks_are_rejected_without_touching_their_target() {
    let fixture = Fixture::new();
    fixture.seed();
    let outside = fixture.0.join("outside");
    fs::write(&outside, b"outside data").unwrap();
    std::os::unix::fs::symlink(&outside, fixture.legacy().join("linked")).unwrap();
    assert!(relocate(&fixture.files(), &fixture.legacy()).is_err());
    assert!(!local_data_dir_in_base_dir(&fixture.files()).exists());
    assert_eq!(fs::read(outside).unwrap(), b"outside data");
}
