//! Dependency-graph gates of the v0.2 design (`docs/design/v0.2-architecture.md`, 阶段与门禁):
//! the Web build links no SQLite, and the protocol crate depends on no infrastructure.

use std::path::Path;
use std::process::Command;

/// `cargo tree` of the workspace with normal (non-dev, non-build) edges.
fn cargo_tree(args: &[&str]) -> String {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = Command::new(env!("CARGO"))
        .args(["tree", "--locked", "-e", "normal"])
        .args(args)
        .current_dir(workspace)
        .output()
        .expect("run cargo tree");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn the_web_build_links_no_sqlite() {
    for crate_name in ["libsqlite3-sys", "rusqlite"] {
        let path =
            cargo_tree(&["-p", "rssr-app", "--target", "wasm32-unknown-unknown", "-i", crate_name]);
        assert!(
            path.trim().is_empty(),
            "the wasm32 build of rssr-app reaches {crate_name}:\n{path}"
        );
    }
    // The check can fail: the native builds do link SQLite.
    let native = cargo_tree(&["-p", "rssr-app", "--target", "all", "-i", "libsqlite3-sys"]);
    assert!(native.contains("rssr-app"), "{native}");
}

#[test]
fn the_api_crate_depends_on_no_infrastructure() {
    let tree = cargo_tree(&["-p", "rssr-api", "--target", "all", "--prefix", "none"]);
    assert!(tree.contains("rssr-application"), "{tree}");
    for forbidden in ["rssr-infra", "rusqlite", "reqwest", "dioxus", "axum"] {
        assert!(!tree.contains(forbidden), "rssr-api depends on {forbidden}:\n{tree}");
    }
}
