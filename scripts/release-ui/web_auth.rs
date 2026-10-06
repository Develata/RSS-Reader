//! Lifecycle for the deployment authentication smoke. HTTP assertions remain in
//! one shell adapter until their separately measured migration.
use crate::process::{self, Cancellation, LogTail, OwnedProcess};
use std::{
    fs::{File, OpenOptions},
    io::{self, Write},
    path::Path,
    process::Command,
    sync::atomic::Ordering,
    thread,
    time::{Duration, Instant},
};

pub fn spawn(root: &Path, profile: &str, port: u16, logs: &Path) -> io::Result<OwnedProcess> {
    let log = logs.join("rssr-web.log");
    File::create(&log)?;
    let mut command = Command::new("cargo");
    // profile selects only the static bundle. The server has always used the
    // default Cargo profile, including when the bundle is a release build.
    command
        .current_dir(root)
        .args(["run", "--locked", "-p", "rssr-web"])
        .env("RSS_READER_WEB_BIND", format!("127.0.0.1:{port}"))
        .env("RSS_READER_WEB_STATIC_DIR", format!("target/dx/rssr-app/{profile}/web/public"))
        .env("RSS_READER_WEB_USERNAME", "smoke")
        .env("RSS_READER_WEB_PASSWORD", "smoke-pass-123")
        .env("RSS_READER_WEB_SESSION_SECRET", "release-ui-regression-session-secret-0123456789")
        .env("RSS_READER_WEB_AUTH_STATE_FILE", logs.join("rssr-web-auth.json"));
    OwnedProcess::spawn(command, &log)
}

fn alive(service: &mut OwnedProcess, phase: &str) -> io::Result<()> {
    if let Some(status) = service.try_wait()? {
        return Err(io::Error::other(format!(
            "rssr-web exited {phase} (exit {}); see rssr-web.log",
            process::exit_code(status)
        )));
    }
    Ok(())
}

// Poll owned requests/assertions alongside the service. No child output pipe
// participates in lifecycle decisions; an unread console cannot block cleanup.
fn wait(
    child: &mut OwnedProcess,
    service: &mut OwnedProcess,
    tail: &mut LogTail,
    cancelled: &Cancellation,
    phase: &str,
) -> io::Result<i32> {
    // Readiness curl usually exits within a few milliseconds. A few early
    // sleeping polls avoid charging it a full 40ms; slow requests return to
    // the existing cadence without busy waiting or changing their timeout.
    let mut early_delays = [5, 10, 20].into_iter();
    loop {
        let code = cancelled.load(Ordering::SeqCst);
        if code != 0 {
            child.stop(code)?;
            tail.drain()?;
            return Ok(code as i32);
        }
        alive(service, phase)?;
        tail.drain()?;
        if let Some(status) = child.try_wait()? {
            child.stop(0)?;
            tail.drain()?;
            return Ok(process::exit_code(status));
        }
        thread::sleep(Duration::from_millis(early_delays.next().unwrap_or(40)));
    }
}

fn retry_delay(service: &mut OwnedProcess, cancelled: &Cancellation) -> io::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(1);
    while Instant::now() < deadline {
        if cancelled.load(Ordering::SeqCst) != 0 {
            return Ok(());
        }
        alive(service, "before readiness")?;
        thread::sleep(
            Duration::from_millis(40).min(deadline.saturating_duration_since(Instant::now())),
        );
    }
    Ok(())
}

pub fn run(
    root: &Path,
    service: &mut OwnedProcess,
    assertions: Command,
    port: u16,
    logs: &Path,
    tail: &mut LogTail,
    cancelled: &Cancellation,
) -> io::Result<i32> {
    let result = (|| {
        let readiness_log = logs.join("rssr-web-readiness.log");
        File::create(&readiness_log)?;
        let mut ready = false;
        // Keep the legacy attempt budget, not a 30-second wall-clock deadline:
        // 30 requests, each connect <=2s / total <=10s, then a 1s failed retry delay.
        for _ in 0..30 {
            let code = cancelled.load(Ordering::SeqCst);
            if code != 0 {
                return Ok(code as i32);
            }
            alive(service, "before readiness")?;
            // Windows' default executable search can select System32/curl.exe
            // before PATH. Resolve the caller's native curl explicitly first.
            #[cfg(windows)]
            let executable = std::env::var_os("PATH")
                .and_then(|paths| {
                    std::env::split_paths(&paths)
                        .map(|directory| root.join(directory).join("curl.exe"))
                        .find(|path| path.is_file())
                })
                .unwrap_or_else(|| "curl".into());
            #[cfg(not(windows))]
            let executable = "curl";
            let mut curl = Command::new(executable);
            curl.current_dir(root);
            curl.args([
                "--connect-timeout",
                "2",
                "--max-time",
                "10",
                "-fsS",
                &format!("http://127.0.0.1:{port}/healthz"),
            ]);
            let mut request = OwnedProcess::spawn(curl, &readiness_log)?;
            let code = wait(&mut request, service, tail, cancelled, "before readiness")?;
            if cancelled.load(Ordering::SeqCst) != 0 {
                return Ok(code);
            }
            if code == 0 {
                ready = true;
                break;
            }
            retry_delay(service, cancelled)?;
        }
        let code = cancelled.load(Ordering::SeqCst);
        if code != 0 {
            return Ok(code as i32);
        }
        if !ready {
            return Err(io::Error::other(
                "rssr-web did not become ready; see rssr-web.log and rssr-web-readiness.log",
            ));
        }
        alive(service, "before assertions")?;
        writeln!(
            OpenOptions::new().append(true).open(logs.join("rssr-web-auth-smoke.log"))?,
            "rssr-web ready; running authentication assertions"
        )?;
        let mut child = OwnedProcess::spawn(assertions, &logs.join("rssr-web-auth-smoke.log"))?;
        // Preserve the assertion adapter's curl/grep exit code if the service
        // dies during a request. Its bounded HTTP calls already fail naturally.
        let code = process::wait(&mut child, tail, cancelled)?;
        if code == 0 {
            alive(service, "before completion")?;
        }
        Ok(code)
    })();
    // Service and assertion processes have distinct owned groups/jobs and logs.
    // Always clean the service, including assertion failure and cancellation.
    let cleanup = service.stop(cancelled.load(Ordering::SeqCst));
    if let Err(error) = &result {
        writeln!(
            OpenOptions::new().append(true).open(logs.join("rssr-web-auth-smoke.log"))?,
            "{error}"
        )?;
    }
    cleanup?;
    result
}
