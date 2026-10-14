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

/// The package names in a `--prefix none` tree.
fn packages(tree: &str) -> Vec<&str> {
    tree.lines().filter_map(|line| line.split_whitespace().next()).collect()
}

#[test]
fn the_web_build_links_no_sqlite() {
    for crate_name in ["libsqlite3-sys", "rusqlite"] {
        let path =
            cargo_tree(&["-p", "rssr-app", "--target", "wasm32-unknown-unknown", "-i", crate_name]);
        assert!(
            path.trim().is_empty(),
            "the wasm32 build of rssr-app reaches {crate_name}:
{path}"
        );
    }
    // The check can fail: the build for this host does link SQLite.
    let native = cargo_tree(&["-p", "rssr-app", "-i", "libsqlite3-sys", "--prefix", "none"]);
    assert!(packages(&native).contains(&"rssr-app"), "{native}");
}

#[test]
fn the_api_crate_depends_on_no_infrastructure() {
    let tree = cargo_tree(&["-p", "rssr-api", "--target", "all", "--prefix", "none"]);
    let packages = packages(&tree);
    assert!(packages.contains(&"rssr-application"), "{tree}");
    for forbidden in
        ["rssr-infra", "rssr-app", "rusqlite", "libsqlite3-sys", "reqwest", "dioxus", "axum"]
    {
        assert!(
            !packages.contains(&forbidden),
            "rssr-api depends on {forbidden}:
{tree}"
        );
    }
    // Direct dependencies are exactly the application layer and a few small crates.
    let direct = cargo_tree(&["-p", "rssr-api", "--depth", "1", "--prefix", "none"]);
    let mut direct: Vec<&str> = self::packages(&direct).into_iter().skip(1).collect();
    direct.sort_unstable();
    direct.dedup();
    // `serde` for the protocol types (`serde_json` is a test-only dependency, not listed).
    assert_eq!(
        direct,
        ["anyhow", "async-trait", "rssr-application", "rssr-domain", "serde", "url"]
    );
}
