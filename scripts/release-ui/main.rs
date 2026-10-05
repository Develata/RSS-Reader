mod plan;
mod process;
mod web_auth;

use clap::Parser;
use plan::{Options, Step};
use process::{Cancellation, Console, LogTail, OwnedProcess};
use serde::Serialize;
use std::{
    collections::HashSet,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::Ordering,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum State {
    Pending,
    Skipped,
    Running,
    Serving,
    Passed,
    Reused,
    Failed,
    Interrupted,
    Blocked,
    Partial,
}
impl State {
    fn label(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Skipped => "skipped",
            Self::Running => "running",
            Self::Serving => "serving (manual checks required)",
            Self::Passed => "passed",
            Self::Reused => "reused (--skip-build)",
            Self::Failed => "failed",
            Self::Interrupted => "interrupted",
            Self::Blocked => "blocked",
            Self::Partial => "partial (external feed skipped)",
        }
    }
}

#[derive(Serialize)]
struct ResultStep {
    #[serde(flatten)]
    step: Step,
    status: State,
    exit_code: Option<i32>,
    duration_ms: u128,
    pid: Option<u32>,
    detail: String,
}

#[derive(Serialize)]
struct Report {
    schema_version: u8,
    started_unix_ms: u128,
    commit: String,
    profile: &'static str,
    port: u16,
    web_port: u16,
    log_dir: PathBuf,
    outcome: &'static str,
    exit_code: Option<i32>,
    external_feed_skipped: bool,
    steps: Vec<ResultStep>,
}

impl Report {
    fn group(&self, group: &str) -> State {
        let states: Vec<_> =
            self.steps.iter().filter(|s| s.step.group == group).map(|s| s.status).collect();
        for priority in [
            State::Failed,
            State::Interrupted,
            State::Serving,
            State::Running,
            State::Blocked,
            State::Pending,
        ] {
            if states.contains(&priority) {
                return priority;
            }
        }
        if group == "fixed" && self.external_feed_skipped {
            return State::Partial;
        }
        if states.iter().all(|s| *s == State::Skipped) {
            State::Skipped
        } else if states.contains(&State::Reused) {
            State::Reused
        } else {
            State::Passed
        }
    }

    fn write(&self) -> io::Result<()> {
        // Persist after every transition; terminal failures never remain pending.
        fs::write(self.log_dir.join("summary.json"), serde_json::to_vec_pretty(self)?)?;
        let mut summary = format!(
            "# 发布前 UI 预检结果\n\n- started (Unix ms)：{}\n- commit：{}\n- profile：{}\n- 静态 Web 端口：{}\n- rssr-web 端口：{}\n- 日志目录：{}\n- outcome：{}\n- exit code：{:?}\n\n## 状态\n\n",
            self.started_unix_ms,
            self.commit,
            self.profile,
            self.port,
            self.web_port,
            self.log_dir.display(),
            self.outcome,
            self.exit_code
        );
        for (label, group) in [
            ("自动化门禁", "automated"),
            ("browser / wasm contract harness", "browser"),
            ("Web bundle", "bundle"),
            ("rssr-web smoke", "web"),
            ("固定 smoke 套件", "fixed"),
            ("静态 Web + SPA fallback", "spa"),
        ] {
            summary.push_str(&format!("- {label}：{}\n", self.group(group).label()));
        }
        summary.push_str("\n## 阶段与日志\n\n| stage | status | exit | ms | log | detail |\n| --- | --- | --- | --- | --- | --- |\n");
        for stage in &self.steps {
            summary.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} |\n",
                stage.step.name,
                stage.status.label(),
                stage.exit_code.map_or(String::new(), |c| c.to_string()),
                stage.duration_ms,
                stage.step.log,
                stage.detail.replace(['\n', '\r', '|'], " ")
            ));
        }
        summary.push_str("\n## 日志与产物\n\n- rssr-web 服务日志：rssr-web.log\n- 固定 smoke 目录：static-web-reader-theme-matrix / static-web-small-viewport-smoke / rssr-web-proxy-feed-smoke / rssr-web-browser-feed-smoke\n\n## 结果记录补充\n\n- 执行环境：\n- env-limited 项：\n- host / sqlite contract harness：\n- wasm / browser contract harness：\n- /entries：\n- /feeds：\n- /settings：\n- /reader/{entry_id}：\n- 静态 reader seed smoke：\n- 默认主题：\n- Atlas Sidebar：\n- Newsprint：\n- Amethyst Glass：\n- Midnight Ledger：\n- 是否允许发布（需手工结论）：\n");
        fs::write(self.log_dir.join("summary.md"), summary)
    }
}

fn unix_ms() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis()
}

fn revision(root: &Path, log_dir: &Path, cancelled: &Cancellation) -> String {
    let read = || -> io::Result<String> {
        // Git handles gitfiles, common refs, packed refs, and detached HEAD.
        // Do not accidentally discover a parent repository for a source snapshot.
        if !root.join(".git").exists() {
            return Err(io::Error::other("no .git entry"));
        }
        let log = log_dir.join("git-revision.log");
        File::create(&log)?;
        let mut command = Command::new("git");
        command.current_dir(root).args(["rev-parse", "--verify", "HEAD"]);
        for name in ["GIT_DIR", "GIT_WORK_TREE", "GIT_COMMON_DIR"] {
            command.env_remove(name);
        }
        let mut child = OwnedProcess::spawn(command, &log)?;
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let signal = cancelled.load(Ordering::SeqCst);
            if signal != 0 || Instant::now() >= deadline {
                child.stop(signal)?;
                return Err(io::Error::other("Git revision probe cancelled or timed out"));
            }
            if let Some(status) = child.try_wait()? {
                child.stop(0)?;
                let hash = fs::read_to_string(&log)?.trim().to_string();
                if status.success()
                    && [40, 64].contains(&hash.len())
                    && hash.bytes().all(|b| b.is_ascii_hexdigit())
                {
                    return Ok(hash);
                }
                return Err(io::Error::other("git rev-parse failed"));
            }
            thread::sleep(Duration::from_millis(20));
        }
    };
    read().unwrap_or_else(|error| format!("unknown ({error})"))
}

fn available_port(port: u16) -> io::Result<()> {
    TcpListener::bind(("127.0.0.1", port))
        .map(drop)
        .map_err(|e| io::Error::new(e.kind(), format!("port {port} is unavailable: {e}")))
}

fn path_arg(path: &Path) -> String {
    let text = path.to_string_lossy();
    #[cfg(windows)]
    {
        text.trim_start_matches(r"\\?\").replace('\\', "/")
    }
    #[cfg(not(windows))]
    {
        text.into_owned()
    }
}

fn bash_path(options: &Options) -> io::Result<PathBuf> {
    if let Some(path) = &options.bash {
        return Ok(path.clone());
    }
    if let Some(path) = std::env::var_os("RSSR_BASH") {
        return Ok(path.into());
    }
    if cfg!(windows) {
        Err(io::Error::other(
            "Bash stages require Git Bash: set RSSR_BASH to its bash.exe or use the .sh entry point (Windows system32/bash.exe is WSL)",
        ))
    } else {
        Ok("bash".into())
    }
}

fn command(step: &Step, options: &Options, root: &Path, log_dir: &Path) -> io::Result<Command> {
    let mut command = if step.program == "bash" {
        let mut command = Command::new(bash_path(options)?);
        #[cfg(unix)]
        command.args(["--noprofile", "--norc"]);
        // Only Windows needs fallback Git Bash utilities when started from
        // PowerShell. Preserve caller PATH priority and the chosen interpreter,
        // including nested bare `bash` calls in the existing stage scripts.
        #[cfg(windows)]
        command.args([
            "--noprofile",
            "--norc",
            "-c",
            "export PATH=\"$PATH:/usr/bin:/bin\"; bash() { \"$BASH\" \"$@\"; }; export -f bash; exec \"$BASH\" \"$@\"",
            "release-ui",
        ]);
        command
    } else {
        Command::new(step.program)
    };
    #[cfg(windows)]
    if step.program == "bash" {
        // Git Bash otherwise rewrites curl's literal form field next=/feeds to a
        // Windows filesystem path. Keep path conversion enabled for file args.
        let mut exclusions = std::env::var("MSYS2_ARG_CONV_EXCL").unwrap_or_default();
        if exclusions != "*" {
            if !exclusions.is_empty() {
                exclusions.push(';');
            }
            exclusions.push_str("next=");
        }
        command.env("MSYS2_ARG_CONV_EXCL", exclusions);
    }
    command
        .current_dir(root)
        .args(step.args.iter().map(|arg| arg.replace("{log-dir}", &path_arg(log_dir))));
    Ok(command)
}

fn spa_ready(port: u16) -> bool {
    let address: SocketAddr = ([127, 0, 0, 1], port).into();
    let Ok(mut stream) = TcpStream::connect_timeout(&address, Duration::from_millis(200)) else {
        return false;
    };
    let timeout = Some(Duration::from_millis(200));
    if stream.set_read_timeout(timeout).is_err() || stream.set_write_timeout(timeout).is_err() {
        return false;
    }
    if stream.write_all(b"GET /entries HTTP/1.0\r\nHost: 127.0.0.1\r\n\r\n").is_err() {
        return false;
    }
    let mut bytes = [0; 128];
    let Ok(count) = stream.read(&mut bytes) else {
        return false;
    };
    let response = String::from_utf8_lossy(&bytes[..count]);
    response.starts_with("HTTP/1.0 200 ") || response.starts_with("HTTP/1.1 200 ")
}

fn execute(
    index: usize,
    report: &mut Report,
    options: &Options,
    root: &Path,
    cancelled: &Cancellation,
    console: &Console,
) -> io::Result<i32> {
    let step = report.steps[index].step.clone();
    let public = root.join(format!("target/dx/rssr-app/{}/web/public", options.profile()));
    if step.name == "web-bundle" && options.skip_build {
        if !public.is_dir() {
            return Err(io::Error::other(format!(
                "Web build output not found: {}",
                public.display()
            )));
        }
        report.steps[index].status = State::Reused;
        return Ok(0);
    }
    if let Some(port) = step.port {
        available_port(port)?;
    }
    if step.name == "small-viewport" {
        available_port(step.port.unwrap() + 10000)?;
    }
    let log = report.log_dir.join(step.log);
    let mut tail = LogTail::open(&log, console)?;
    writeln!(OpenOptions::new().append(true).open(&log)?, "\n=== {} ===", step.name)?;
    if step.name == "web-auth" {
        let assertions = Step {
            program: "bash",
            args: vec![
                "scripts/run_rssr_web_auth_assertions.sh".into(),
                options.web_port.to_string(),
                "{log-dir}".into(),
            ],
            ..step
        };
        let assertions = command(&assertions, options, root, &report.log_dir)?;
        let mut service =
            web_auth::spawn(root, options.profile(), options.web_port, &report.log_dir)?;
        report.steps[index].pid = Some(service.id());
        report.write()?;
        return web_auth::run(
            root,
            &mut service,
            assertions,
            options.web_port,
            &report.log_dir,
            &mut tail,
            cancelled,
        );
    }
    let mut child = OwnedProcess::spawn(command(&step, options, root, &report.log_dir)?, &log)?;
    report.steps[index].pid = Some(child.id());
    report.write()?;
    if step.name == "spa" {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            tail.drain()?;
            if let Some(status) = child.try_wait()? {
                child.stop(0)?;
                let code = process::exit_code(status);
                report.steps[index].detail = format!("SPA exited before readiness (exit {code})");
                return Ok(if code == 0 { 1 } else { code });
            }
            if cancelled.load(Ordering::SeqCst) != 0 {
                return process::wait(&mut child, &mut tail, cancelled);
            }
            if spa_ready(options.port) && child.try_wait()?.is_none() {
                break;
            }
            if Instant::now() >= deadline {
                return Err(io::Error::other("SPA did not become ready within 30 seconds"));
            }
            thread::sleep(Duration::from_millis(80));
        }
        report.steps[index].status = State::Serving;
        report.write()?;
        console.send(false, format!(
            "SPA ready at http://127.0.0.1:{}/entries; manually check /feeds, /settings and /__codex/setup-local-auth?username=smoke&password=smoke-pass-123&seed=reader-demo&next=/entries/2. Ctrl+C stops it.\n",
            options.port
        ).as_bytes());
    }
    let code = process::wait(&mut child, &mut tail, cancelled)?;
    if step.name == "web-bundle" && code == 0 && !public.is_dir() {
        return Err(io::Error::other(format!("Web build output not found: {}", public.display())));
    }
    if step.name == "spa" && cancelled.load(Ordering::SeqCst) == 0 && code == 0 {
        return Err(io::Error::other("SPA exited unexpectedly after readiness"));
    }
    Ok(code)
}

fn run(options: Options, console: &Console) -> io::Result<i32> {
    let steps = plan::build(&options);
    let root = options
        .repo_root
        .clone()
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .canonicalize()?;
    if !root.join("scripts/run_web_spa_regression_server.sh").is_file() {
        return Err(io::Error::other("--repo-root must point to RSS-Reader"));
    }
    let log_dir = root.join(options.log_dir.clone().unwrap_or_else(|| {
        PathBuf::from(format!("target/release-ui-regression/{}-{}", unix_ms(), std::process::id()))
    }));
    fs::create_dir_all(&log_dir)?;
    let mut report = Report {
        schema_version: 1,
        started_unix_ms: unix_ms(),
        commit: "unknown (revision probe pending)".into(),
        profile: options.profile(),
        port: options.port,
        web_port: options.web_port,
        log_dir,
        outcome: "running",
        exit_code: None,
        external_feed_skipped: options.skip_external_feed,
        steps: steps
            .into_iter()
            .map(|step| ResultStep {
                status: if step.enabled { State::Pending } else { State::Skipped },
                exit_code: None,
                duration_ms: 0,
                pid: None,
                detail: if step.name == "proxy-feed" && options.skip_external_feed {
                    "external availability not tested (--skip-external-feed); local browser feed fixture remains enabled".into()
                } else { String::new() },
                step,
            })
            .collect(),
    };
    report.write()?;
    let cancelled = match process::install_handlers() {
        Ok(cancelled) => cancelled,
        Err(error) => {
            report.outcome = "failed";
            report.exit_code = Some(1);
            for stage in &mut report.steps {
                if stage.status == State::Pending {
                    stage.status = State::Blocked;
                    stage.detail = format!("process supervisor setup failed: {error}");
                }
            }
            report.write()?;
            return Err(error);
        }
    };
    report.commit = revision(&root, &report.log_dir, &cancelled);
    let mut initialized_logs = HashSet::new();
    let mut code = 0;
    for index in 0..report.steps.len() {
        if !report.steps[index].step.enabled {
            continue;
        }
        let start = Instant::now();
        report.steps[index].status = State::Running;
        report.write()?;
        let result = (|| {
            let signal = cancelled.load(Ordering::SeqCst);
            if signal != 0 {
                return Ok(signal as i32);
            }
            let log = report.steps[index].step.log;
            if initialized_logs.insert(log) {
                File::create(report.log_dir.join(log))?;
            }
            console
                .send(false, format!("Running {}...\n", report.steps[index].step.name).as_bytes());
            execute(index, &mut report, &options, &root, &cancelled, console)
        })();
        let interrupted = cancelled.load(Ordering::SeqCst) != 0;
        let stage = &mut report.steps[index];
        stage.duration_ms = start.elapsed().as_millis();
        code = match result {
            Ok(code) => code,
            Err(error) => {
                stage.detail = error.to_string();
                console.send(true, format!("{}: {error}\n", stage.step.name).as_bytes());
                1
            }
        };
        if interrupted && code == 0 {
            code = cancelled.load(Ordering::SeqCst) as i32;
        }
        stage.exit_code = Some(code);
        if interrupted {
            stage.status = State::Interrupted;
        } else if code != 0 {
            stage.status = State::Failed;
        } else if stage.status != State::Reused {
            stage.status = State::Passed;
        }
        if code != 0 || interrupted {
            for pending in &mut report.steps[index + 1..] {
                if pending.status == State::Pending {
                    pending.status = State::Blocked;
                }
            }
            report.outcome = if interrupted { "interrupted" } else { "failed" };
            break;
        }
        report.write()?;
    }
    if code == 0 {
        code = cancelled.load(Ordering::SeqCst) as i32;
        report.outcome = if code != 0 {
            "interrupted"
        } else if options.skip_external_feed {
            "completed-with-skips"
        } else {
            "completed"
        };
    }
    report.exit_code = Some(code);
    report.write()?;
    console.send(
        false,
        format!("Summary written to {}\n", report.log_dir.join("summary.md").display()).as_bytes(),
    );
    Ok(code)
}

fn main() {
    let options = match Options::try_parse() {
        Ok(options) => options,
        Err(error) => {
            let code = if error.use_stderr() { 1 } else { 0 };
            let _ = error.print();
            std::process::exit(code);
        }
    };
    if let Err(error) = options.validate() {
        eprintln!("{error}");
        std::process::exit(1);
    }
    if options.plan {
        println!(
            "{}",
            serde_json::to_string_pretty(&plan::build(&options)).expect("serializable plan")
        );
        return;
    }
    let console = match Console::new() {
        Ok(console) => console,
        Err(error) => {
            eprintln!("release-ui: {error}");
            std::process::exit(1);
        }
    };
    let code = match run(options, &console) {
        Ok(code) => code,
        Err(error) => {
            console.send(true, format!("release-ui: {error}\n").as_bytes());
            1
        }
    };
    console.finish();
    std::process::exit(code);
}
