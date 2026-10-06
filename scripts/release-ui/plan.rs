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
    /// Run deterministic local feed fixtures; exclude the external proxy probe explicitly.
    #[arg(long)]
    pub skip_external_feed: bool,
    #[arg(long)]
    pub no_serve: bool,
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
        if self.skip_external_feed && !self.full && !self.with_fixed_smokes {
            return Err("--skip-external-feed requires --full or --with-fixed-smokes".into());
        }
        if self.full || self.with_fixed_smokes {
            if self.port > 55524 {
                return Err("--port must be <= 55524 with fixed smokes (small viewport uses port + 11 + 10000)".into());
            }
            if self.web_port > 65524 {
                return Err("--web-port must be <= 65524 with fixed smokes".into());
            }
        } else if self.with_rssr_web && self.web_port == 65535 {
            return Err("--web-port must be <= 65534 with rssr-web browser smoke".into());
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
            "test --locked -p rssr-infra --test test_refresh_contract_harness --test test_subscription_contract_harness --test test_config_exchange_contract_harness",
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
            "scripts/run_wasm_contract_harness.sh wasm_refresh_contract_harness wasm_subscription_contract_harness wasm_config_exchange_contract_harness",
        ),
        "browser-contracts.log",
        None,
    );
    let mut build_args = words("build --platform web --package rssr-app --locked");
    if options.release {
        build_args.push("--release".into());
    }
    add(
        "web-bundle",
        "bundle",
        web || fixed || !options.no_serve,
        "dx",
        build_args,
        "web-build.log",
        None,
    );
    let auth = web_auth(options);
    add(auth.name, auth.group, auth.enabled, auth.program, auth.args, auth.log, auth.port);
    let smoke = |script: &str, port: u16, directory: &str| {
        vec![
            format!("scripts/{script}.sh"),
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
        smoke(
            "run_rssr_web_browser_feed_smoke",
            options.web_port.saturating_add(1),
            "rssr-web-browser-feed-smoke",
        ),
        "rssr-web-browser-feed-smoke.log",
        Some(options.web_port.saturating_add(1)),
    );
    for (name, script, port, directory) in [
        (
            "reader-theme-matrix",
            "run_static_web_reader_theme_matrix",
            options.port.saturating_add(10),
            "static-web-reader-theme-matrix",
        ),
        (
            "small-viewport",
            "run_static_web_small_viewport_smoke",
            options.port.saturating_add(11),
            "static-web-small-viewport-smoke",
        ),
        (
            "proxy-feed",
            "run_rssr_web_proxy_feed_smoke",
            options.web_port.saturating_add(10),
            "rssr-web-proxy-feed-smoke",
        ),
        (
            "fixed-browser-feed",
            "run_rssr_web_browser_feed_smoke",
            options.web_port.saturating_add(11),
            "rssr-web-browser-feed-smoke",
        ),
    ] {
        add(
            name,
            "fixed",
            fixed && !(name == "proxy-feed" && options.skip_external_feed),
            "bash",
            smoke(script, port, directory),
            "fixed-smokes.log",
            Some(port),
        );
    }
    add(
        "spa",
        "spa",
        !options.no_serve,
        "bash",
        vec![
            "scripts/run_web_spa_regression_server.sh".into(),
            "--skip-build".into(),
            format!("--{}", options.profile()),
            "--port".into(),
            options.port.to_string(),
        ],
        "spa-server.log",
        Some(options.port),
    );
    steps
}

#[cfg(test)]
mod tests {
    use super::*;
    fn opts(args: &[&str]) -> Options {
        Options::try_parse_from(std::iter::once("release-ui").chain(args.iter().copied())).unwrap()
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
        assert!(opts(&["--full", "--web-port", "65525"]).validate().is_err());
        assert!(opts(&["--with-rssr-web", "--web-port", "65535"]).validate().is_err());
        assert!(opts(&["--full", "--port", "55524", "--web-port", "65524"]).validate().is_ok());
    }
    #[test]
    fn combinations_preserve_order_build_once_and_browser_deduplication() {
        for mask in 0..128 {
            let flags = [
                "--skip-automated",
                "--skip-build",
                "--no-serve",
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
                usize::from(web || fixed || !options.no_serve)
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
                if ["web", "fixed", "spa"].contains(&step.group) {
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
                "proxy-feed",
                "fixed-browser-feed",
                "spa"
            ]
        );
    }

    #[test]
    fn external_probe_is_only_excluded_by_explicit_selection() {
        assert!(opts(&["--skip-external-feed"]).validate().is_err());
        let options = opts(&["--full", "--skip-external-feed", "--no-serve"]);
        assert!(options.validate().is_ok());
        let steps = build(&options);
        assert!(!steps.iter().find(|s| s.name == "proxy-feed").unwrap().enabled);
        assert!(steps.iter().find(|s| s.name == "fixed-browser-feed").unwrap().enabled);
        assert!(build(&opts(&["--full"])).iter().find(|s| s.name == "proxy-feed").unwrap().enabled);
    }
}
