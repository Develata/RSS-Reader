// The build identifier, shared by the build scripts of the server (`rssr-web`) and the Web page
// (`rssr-app`) through `include!`, so both compute it the same way (v0.2 phase C, C3). The server
// refuses writes from a page of another build (`docs/design/v0.2-phase-c-plan.md`).
//
// - `RSSR_BUILD_ID` when set (releases pass the tag; an image build must pass it, since the build
//   context has no Git metadata);
// - otherwise `dev-<commit>`, or `dev-<commit>-dirty-<hash>` when the work tree has changes
//   (`<hash>` covers the diff and the untracked files, so two builds of different changes differ);
// - `dev-unknown` without Git.
//
// Never `CARGO_PKG_VERSION`: it stays at 0.1.0.

/// Sets `RSSR_BUILD_ID` for the crate being built (`env!("RSSR_BUILD_ID")`) and tells Cargo when
/// to compute it again.
fn emit_build_id() {
    println!("cargo:rerun-if-env-changed=RSSR_BUILD_ID");
    let id = match std::env::var("RSSR_BUILD_ID") {
        Ok(id) if !id.trim().is_empty() => id.trim().to_owned(),
        _ => dev_build_id(),
    };
    assert!(
        id.bytes().all(|byte| byte.is_ascii_graphic()),
        "RSSR_BUILD_ID must be printable ASCII without spaces: {id:?}"
    );
    println!("cargo:rustc-env=RSSR_BUILD_ID={id}");
}

fn dev_build_id() -> String {
    let manifest_dir = std::path::PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("Cargo sets CARGO_MANIFEST_DIR"),
    );
    let git_in = |dir: &std::path::Path, args: &[&str]| -> Option<Vec<u8>> {
        let output =
            std::process::Command::new("git").arg("-C").arg(dir).args(args).output().ok()?;
        output.status.success().then_some(output.stdout)
    };
    let git = |args: &[&str]| git_in(&manifest_dir, args);
    let text = |bytes: Vec<u8>| String::from_utf8_lossy(&bytes).trim().to_owned();

    // Without Git, only the variable and this file can change the identifier.
    println!("cargo:rerun-if-changed={}", manifest_dir.join("build.rs").display());
    let Some(commit) = git(&["rev-parse", "--short=12", "HEAD"]).map(text) else {
        return "dev-unknown".to_owned();
    };

    // Compute it again when the commit, the index or a source file changes (a directory is
    // scanned as a whole).
    let mut watched = vec!["HEAD".to_owned(), "index".to_owned(), "packed-refs".to_owned()];
    if let Some(branch) = git(&["symbolic-ref", "-q", "HEAD"]).map(text) {
        watched.push(branch);
    }
    for name in watched {
        if let Some(path) = git(&["rev-parse", "--git-path", &name]).map(text) {
            println!("cargo:rerun-if-changed={}", manifest_dir.join(path).display());
        }
    }
    let Some(root) = git(&["rev-parse", "--show-toplevel"]).map(text) else {
        return format!("dev-{commit}");
    };
    let root = std::path::PathBuf::from(root);
    for watched in ["crates", "build-support", "Cargo.toml", "Cargo.lock"] {
        println!("cargo:rerun-if-changed={}", root.join(watched).display());
    }

    // The whole work tree, whatever the crate's directory.
    let diff = git_in(&root, &["diff", "HEAD", "--binary"]).unwrap_or_default();
    let untracked =
        git_in(&root, &["ls-files", "--others", "--exclude-standard", "-z"]).unwrap_or_default();
    if diff.is_empty() && untracked.is_empty() {
        return format!("dev-{commit}");
    }
    // FNV-1a: stable across toolchains and platforms, which the standard hashers are not.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut feed = |bytes: &[u8]| {
        for &byte in bytes {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    feed(&diff);
    for name in untracked.split(|&byte| byte == 0).filter(|name| !name.is_empty()) {
        feed(name);
        feed(&[0]);
        let path = root.join(String::from_utf8_lossy(name).as_ref());
        feed(&std::fs::read(path).unwrap_or_default());
        feed(&[0]);
    }
    format!("dev-{commit}-dirty-{:012x}", hash & 0xffff_ffff_ffff)
}
