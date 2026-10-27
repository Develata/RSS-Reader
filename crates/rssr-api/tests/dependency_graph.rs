//! Dependency-graph gates of the v0.2 design (`docs/design/v0.2-architecture.md`, 阶段与门禁):
//! the Web build links no SQLite and, outside the Pages demo, no library kept in the browser; the
//! protocol crate depends on no infrastructure.

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
    let tree =
        cargo_tree(&["-p", "rssr-app", "--target", "wasm32-unknown-unknown", "--prefix", "none"]);
    let names = packages(&tree);
    for crate_name in ["libsqlite3-sys", "rusqlite", "sqlite-wasm-rs"] {
        assert!(
            !names.contains(&crate_name),
            "the wasm32 build of rssr-app reaches {crate_name}:\n{tree}"
        );
    }
    // The check can fail: the build for this host does link SQLite.
    let native = cargo_tree(&["-p", "rssr-app", "-i", "libsqlite3-sys", "--prefix", "none"]);
    assert!(packages(&native).contains(&"rssr-app"), "{native}");
}

/// The features resolved for `rssr-infra` in the wasm32 build of `rssr-app` with `extra` options.
fn infra_features_of_the_web_build(extra: &[&str]) -> Vec<String> {
    let mut args = vec![
        "-p",
        "rssr-app",
        "--target",
        "wasm32-unknown-unknown",
        "--prefix",
        "none",
        "--format",
        "{p} | {f}",
    ];
    args.extend_from_slice(extra);
    let tree = cargo_tree(&args);
    let line = tree.lines().find(|line| line.starts_with("rssr-infra ")).expect("rssr-infra");
    let features = line.split_once(" | ").map_or("", |(_, features)| features);
    features.split(',').filter(|feature| !feature.is_empty()).map(str::to_owned).collect()
}

/// The formal Web build talks to the server: it has `http-backend` and not the library kept in
/// page (`memory-store`); the Pages demo links SQLite through its WASM backend (phase D). This shows what is
/// built; which backend starts is shown by the page's start-up code and the browser regression.
#[test]
fn only_pages_links_the_memory_sqlite_store() {
    let formal = infra_features_of_the_web_build(&[]);
    assert!(formal.iter().any(|feature| feature == "http-backend"), "{formal:?}");
    assert!(!formal.iter().any(|feature| feature == "memory-store"), "{formal:?}");
    let demo = infra_features_of_the_web_build(&["--features", "pages-demo"]);
    for feature in ["http-backend", "memory-store"] {
        assert!(demo.iter().any(|present| present == feature), "{feature}: {demo:?}");
    }
    let demo = cargo_tree(&[
        "-p",
        "rssr-app",
        "--target",
        "wasm32-unknown-unknown",
        "--features",
        "pages-demo",
        "--prefix",
        "none",
    ]);
    let names = packages(&demo);
    assert!(names.contains(&"rusqlite") && names.contains(&"sqlite-wasm-rs"), "{demo}");
    assert!(!names.contains(&"libsqlite3-sys"), "Pages must use the WASM backend: {demo}");
}

#[test]
fn the_api_crate_depends_on_no_infrastructure() {
    let tree = cargo_tree(&["-p", "rssr-api", "--target", "all", "--prefix", "none"]);
    let packages = packages(&tree);
    assert!(packages.contains(&"rssr-application"), "{tree}");
    for forbidden in [
        "rssr-infra",
        "rssr-app",
        "rusqlite",
        "libsqlite3-sys",
        "sqlite-wasm-rs",
        "reqwest",
        "dioxus",
        "axum",
    ] {
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
