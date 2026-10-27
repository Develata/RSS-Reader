use clap::Parser;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    about = "Release UI acceptance runner (existing Bash stages still require Bash)",
    args_override_self = true
)]
pub struct Options {
    #[arg(long, default_value = "8091", value_parser = port)]
    pub port: u16,
    #[arg(long, default_value = "18081", value_parser = port)]
    pub web_port: u16,
    #[arg(long, overrides_with = "release")]
    pub debug: bool,
    #[arg(long, overrides_with = "debug")]
    pub release: bool,
    #[arg(long)]
    pub skip_automated: bool,
    #[arg(long)]
    pub skip_build: bool,
    #[arg(long)]
    pub with_rssr_web: bool,
    /// Run only deployment authentication against an existing static bundle.
    #[arg(long, conflicts_with_all = ["full", "with_rssr_web", "with_fixed_smokes", "with_browser_contracts"])]
    pub web_auth_only: bool,
    #[arg(long)]
    pub with_browser_contracts: bool,
    #[arg(long)]
    pub with_fixed_smokes: bool,
    #[arg(long)]
    pub full: bool,
    #[arg(long, value_parser = nonempty_path)]
    pub log_dir: Option<PathBuf>,
    /// Print the execution plan as JSON without running tools or creating logs.
    #[arg(long)]
    pub plan: bool,
    /// Repository root (defaults to the directory containing this tool's sources).
    #[arg(long)]
    pub repo_root: Option<PathBuf>,
    /// Bash executable; also accepts RSSR_BASH. On Windows select Git Bash explicitly.
    #[arg(long)]
    pub bash: Option<PathBuf>,
}

fn port(value: &str) -> Result<u16, String> {
    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err("expected a decimal port in 1..65535".into());
    }
    value
        .parse::<u16>()
        .ok()
        .filter(|n| *n != 0)
        .ok_or_else(|| "expected a decimal port in 1..65535".into())
}

fn nonempty_path(value: &str) -> Result<PathBuf, String> {
    if value.is_empty() { Err("path must not be empty".into()) } else { Ok(value.into()) }
}

impl Options {
    pub fn profile(&self) -> &'static str {
        if self.release { "release" } else { "debug" }
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.full || self.with_fixed_smokes {
            if self.port > 55524 {
                return Err("--port must be <= 55524 with fixed smokes (the largest smoke uses port + 11, and its Chrome DevTools port adds 10000)".into());
            }
            if self.web_port > 55524 {
                return Err("--web-port must be <= 55524 with fixed smokes (the feed smoke uses web-port + 11, and its Chrome DevTools port adds 10000)".into());
            }
        } else if self.with_rssr_web && self.web_port > 55534 {
            return Err("--web-port must be <= 55534 with the rssr-web browser smoke (it uses web-port + 1, and its Chrome DevTools port adds 10000)".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Step {
    pub name: &'static str,
    pub group: &'static str,
    pub enabled: bool,
    pub program: &'static str,
    pub args: Vec<String>,
    pub log: &'static str,
    pub port: Option<u16>,
}

fn web_auth(options: &Options) -> Step {
    Step {
        name: "web-auth",
        group: "web",
        enabled: options.web_auth_only || options.full || options.with_rssr_web,
        program: "release-ui",
        args: vec![
            "--web-auth-only".into(),
            format!("--{}", options.profile()),
            "--web-port".into(),
            options.web_port.to_string(),
            "--log-dir".into(),
            "{log-dir}".into(),
        ],
        log: "rssr-web-auth-smoke.log",
        port: Some(options.web_port),
    }
}

pub fn build(options: &Options) -> Vec<Step> {
    if options.web_auth_only {
        return vec![web_auth(options)];
    }
    let web = options.full || options.with_rssr_web;
    let fixed = options.full || options.with_fixed_smokes;
    let browser = options.full || options.with_browser_contracts;
    let mut steps = Vec::new();
    let mut add = |name, group, enabled, program, args: Vec<String>, log, port| {
        steps.push(Step { name, group, enabled, program, args, log, port });
    };
    let words = |s: &str| s.split_whitespace().map(String::from).collect::<Vec<_>>();
    add(
        "wasm-check",
        "automated",
        !options.skip_automated,
        "cargo",
        words("check --locked -p rssr-app --target wasm32-unknown-unknown"),
        "automated-gates.log",
        None,
    );
    add(
        "app-tests",
        "automated",
        !options.skip_automated,
        "cargo",
        words("test --locked -p rssr-app"),
        "automated-gates.log",
        None,
    );
    add(
        "host-contracts",
        "automated",
        !options.skip_automated,
        "cargo",
        words(
            "test --locked -p rssr-infra --test test_library_refresh_contract_harness --test test_library_subscription_contract_harness --test test_library_config_exchange_contract_harness",
        ),
        "automated-gates.log",
        None,
    );
    add(
        "web-tests",
        "automated",
        !options.skip_automated,
        "cargo",
        words("test --locked -p rssr-web"),
        "automated-gates.log",
        None,
    );
    add(
        "browser-contracts",
        "browser",
        browser,
        "bash",
        words(
            "scripts/run_wasm_contract_harness.sh test_library_refresh_contract_harness test_library_subscription_contract_harness test_library_config_exchange_contract_harness test_library_contracts test_library_entry_state_and_search test_library_queries test_library_refresh_tickets test_library_persistence_atomicity test_library_application_refresh_store_adapter test_library_settings_repository test_library_unicode_title_search test_opml_interop wasm_memory_store wasm_reader_html_contract_harness",
        ),
        "browser-contracts.log",
        None,
    );
    let mut build_args = words("build --platform web --package rssr-app --locked");
    if options.release {
        // Match CI: retain -Oz optimization, without the Windows DWARF rewrite crash.
        build_args.extend(words("--release --debug-symbols false"));
    }
    add("web-bundle", "bundle", web || fixed, "dx", build_args, "web-build.log", None);
    let auth = web_auth(options);
    add(auth.name, auth.group, auth.enabled, auth.program, auth.args, auth.log, auth.port);
    let smoke = |suite: &str, port: u16, directory: &str| {
        vec![
            "scripts/run_server_web_regression.sh".into(),
            "--suite".into(),
            suite.into(),
            "--skip-build".into(),
            format!("--{}", options.profile()),
            "--port".into(),
            port.to_string(),
            "--log-dir".into(),
            format!("{{log-dir}}/{directory}"),
        ]
    };
    add(
        "web-browser-feed",
        "web",
        web && !fixed,
        "bash",
        smoke("feed-smoke", options.web_port.saturating_add(1), "server-web-feed-smoke"),
        "rssr-web-browser-feed-smoke.log",
        Some(options.web_port.saturating_add(1)),
    );
    for (name, suite, port, directory) in [
        (
            "reader-theme-matrix",
            "theme-matrix",
            options.port.saturating_add(10),
            "server-web-theme-matrix",
        ),
        ("small-viewport", "regression", options.port.saturating_add(11), "server-web-regression"),
        (
            "fixed-browser-feed",
            "feed-smoke",
            options.web_port.saturating_add(11),
            "server-web-feed-smoke",
        ),
    ] {
        add(
            name,
            "fixed",
            fixed,
            "bash",
            smoke(suite, port, directory),
            "fixed-smokes.log",
            Some(port),
        );
    }
    steps
}

#[cfg(test)]
mod tests {
    use super::*;
    fn opts(args: &[&str]) -> Options {
        Options::try_parse_from(std::iter::once("release-ui").chain(args.iter().copied())).unwrap()
    }

    // Fake CLI tools verify orchestration, but cannot detect renamed product test targets.
    #[test]
    fn planned_cargo_tests_exist_in_workspace() {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.toml");
        let output = std::process::Command::new(env!("CARGO"))
            .args(["metadata", "--locked", "--no-deps", "--format-version", "1", "--manifest-path"])
            .arg(manifest)
            .output()
            .expect("read product workspace metadata with Cargo");
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        let metadata: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let packages = metadata["packages"].as_array().unwrap();
        let members = metadata["workspace_members"].as_array().unwrap();
        for step in build(&opts(&["--full"]))
            .into_iter()
            .filter(|step| step.enabled && step.program == "cargo")
        {
            let package_name = &step
                .args
                .windows(2)
                .find(|arg| arg[0] == "-p" || arg[0] == "--package")
                .expect("planned Cargo step must select a package")[1];
            let package = packages
                .iter()
                .find(|package| {
                    package["name"] == *package_name && members.contains(&package["id"])
                })
                .unwrap_or_else(|| {
                    panic!("{}: missing workspace package {package_name}", step.name)
                });
            for target_name in
                step.args.windows(2).filter(|arg| arg[0] == "--test").map(|arg| &arg[1])
            {
                assert!(
                    package["targets"].as_array().unwrap().iter().any(|target| {
                        target["name"] == *target_name
                            && target["kind"].as_array().unwrap().iter().any(|kind| kind == "test")
                    }),
                    "{}: {package_name} has no test target {target_name}",
                    step.name
                );
            }
        }
    }

    #[test]
    fn argument_contract() {
        assert_eq!(opts(&[]).profile(), "debug");
        assert_eq!(opts(&["--release", "--debug"]).profile(), "debug");
        assert_eq!(opts(&["--debug", "--release"]).profile(), "release");
        assert_eq!(opts(&["--port", "08091"]).port, 8091);
        for flag in ["--port", "--web-port", "--log-dir"] {
            assert!(Options::try_parse_from(["release-ui", flag]).is_err());
            assert!(Options::try_parse_from(["release-ui", flag, ""]).is_err());
            assert!(Options::try_parse_from(["release-ui", flag, "--full"]).is_err());
        }
        for value in ["0", "65536", "-1", "1.5", "abc", "+20", " 80"] {
            assert!(Options::try_parse_from(["release-ui", "--port", value]).is_err());
        }
        assert!(Options::try_parse_from(["release-ui", "--unknown"]).is_err());
        assert!(opts(&["--full", "--port", "55525"]).validate().is_err());
        assert!(opts(&["--full", "--web-port", "55525"]).validate().is_err());
        assert!(opts(&["--with-fixed-smokes", "--web-port", "55525"]).validate().is_err());
        assert!(opts(&["--with-rssr-web", "--web-port", "55535"]).validate().is_err());
        assert!(opts(&["--with-rssr-web", "--web-port", "55534"]).validate().is_ok());
        assert!(opts(&["--full", "--port", "55524", "--web-port", "55524"]).validate().is_ok());
        assert!(Options::try_parse_from(["release-ui", "--no-serve"]).is_err());
    }
    #[test]
    fn combinations_preserve_order_build_once_and_browser_deduplication() {
        for mask in 0..64 {
            let flags = [
                "--skip-automated",
                "--skip-build",
                "--with-rssr-web",
                "--with-browser-contracts",
                "--with-fixed-smokes",
                "--full",
            ];
            let selected: Vec<_> = flags
                .iter()
                .enumerate()
                .filter(|(i, _)| mask & (1 << i) != 0)
                .map(|(_, f)| *f)
                .collect();
            let options = opts(&selected);
            let enabled: Vec<_> = build(&options).into_iter().filter(|s| s.enabled).collect();
            let web = options.full || options.with_rssr_web;
            let fixed = options.full || options.with_fixed_smokes;
            assert_eq!(
                enabled.iter().filter(|s| s.name == "web-bundle").count(),
                usize::from(web || fixed)
            );
            assert_eq!(
                enabled
                    .iter()
                    .filter(|s| s.name == "web-browser-feed" || s.name == "fixed-browser-feed")
                    .count(),
                usize::from(web || fixed)
            );
            let bundle = enabled.iter().position(|s| s.name == "web-bundle");
            for (i, step) in enabled.iter().enumerate() {
                if ["web", "fixed"].contains(&step.group) {
                    assert!(bundle.unwrap() < i);
                }
            }
        }
        let names: Vec<_> =
            build(&opts(&["--full"])).into_iter().filter(|s| s.enabled).map(|s| s.name).collect();
        assert_eq!(
            names,
            [
                "wasm-check",
                "app-tests",
                "host-contracts",
                "web-tests",
                "browser-contracts",
                "web-bundle",
                "web-auth",
                "reader-theme-matrix",
                "small-viewport",
                "fixed-browser-feed"
            ]
        );
    }
}
