use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

struct Workspace(PathBuf);

impl Workspace {
    fn new() -> Self {
        // Tests on parallel threads of one process can read the same clock value (2026-10-20,
        // Windows: `AlreadyExists`); the sequence keeps their workspaces apart.
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "rssr-cli-输出 contract-{}-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).expect("clock").as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("create isolated workspace");
        Self(path)
    }

    fn run(&self, arguments: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_rssr-cli"))
            .arg("--data-dir")
            .arg(self.0.join("数据 dir"))
            .args(arguments)
            .output()
            .expect("run actual CLI binary")
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn success(output: Output) -> String {
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(String::from_utf8_lossy(&output.stderr).contains("初始化 CLI 本地资料库"));
    String::from_utf8(output.stdout).expect("UTF-8 stdout")
}

#[test]
fn json_output_is_parseable_and_export_can_be_imported() {
    let workspace = Workspace::new();
    let exported = success(workspace.run(&["export-config"]));
    let config: serde_json::Value = serde_json::from_str(&exported).expect("clean config JSON");
    assert!(config["feeds"].is_array());
    let file = workspace.0.join("配置 backup.json");
    fs::write(&file, exported).expect("write redirected export");
    success(workspace.run(&["import-config", file.to_str().expect("UTF-8 path")]));

    let settings = success(workspace.run(&["show-settings"]));
    serde_json::from_str::<rssr_domain::UserSettings>(&settings).expect("clean settings JSON");
}

#[test]
fn opml_output_can_be_imported_without_filtering_logs() {
    let workspace = Workspace::new();
    let exported = success(workspace.run(&["export-opml"]));
    assert!(exported.starts_with("<?xml"), "stdout must start with the OPML XML document");
    let file = workspace.0.join("订阅 backup.opml");
    fs::write(&file, exported).expect("write redirected export");
    success(workspace.run(&["import-opml", file.to_str().expect("UTF-8 path")]));
}

/// The CLI closes the library before it exits, after a command that succeeded and after one that
/// failed: the log is written back into the main file and removed, so the process never ends in
/// the middle of that (which would leave the main file part-written).
#[test]
fn the_library_is_closed_before_the_cli_exits() {
    let workspace = Workspace::new();
    let library = workspace.0.join("数据 dir").join("library.sqlite3");
    let side_files = || {
        ["-wal", "-shm"]
            .into_iter()
            .filter(|suffix| {
                let mut name = library.as_os_str().to_os_string();
                name.push(suffix);
                PathBuf::from(name).exists()
            })
            .collect::<Vec<_>>()
    };
    success(workspace.run(&["show-settings"]));
    assert!(library.is_file());
    assert_eq!(side_files(), Vec::<&str>::new(), "after a command that succeeded");
    let failed = workspace.run(&["import-config", "missing-config-file.json"]);
    assert!(!failed.status.success());
    assert_eq!(side_files(), Vec::<&str>::new(), "after a command that failed");
}

#[test]
fn failed_command_leaves_stdout_empty_and_exits_nonzero() {
    let workspace = Workspace::new();
    let output = workspace.run(&["import-config", "missing-config-file.json"]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty(), "diagnostics must not become machine-readable output");
    assert!(!output.stderr.is_empty());
}

#[test]
fn refresh_requires_exactly_one_target_before_opening_database() {
    let workspace = Workspace::new();
    for arguments in [vec!["refresh"], vec!["refresh", "--all", "--feed-id", "1"]] {
        let output = workspace.run(&arguments);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
        assert!(
            !workspace.0.join("数据 dir").exists(),
            "no data directory before parsing succeeds"
        );
    }

    let output = workspace.run(&["refresh", "--all"]);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(workspace.0.join("数据 dir").join("library.sqlite3").exists());
}
