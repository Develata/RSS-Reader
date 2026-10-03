// Executable stand-in for cargo/dx/bash. Tests copy this binary under tool names;
// production code has no fixture mode. Native descendants exercise OS cleanup.
use std::{
    env,
    fs::{self, OpenOptions},
    io::{Read, Write},
    net::TcpListener,
    path::Path,
    process::{Command, Stdio},
    thread,
    time::Duration,
};

fn pause() -> ! {
    loop {
        thread::sleep(Duration::from_secs(1));
    }
}

#[allow(clippy::zombie_processes)] // Deliberate orphans: the runner must reap its tree.
fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let pid = std::process::id();
    if args.first().is_some_and(|s| s == "--leaf") {
        let listener = TcpListener::bind((
            "127.0.0.1",
            env::var("FIXTURE_PORT").unwrap().parse::<u16>().unwrap(),
        ))
        .unwrap();
        fs::write("leaf.pid", pid.to_string()).unwrap();
        for connection in listener.incoming() {
            let mut stream = connection.unwrap();
            stream.set_read_timeout(Some(Duration::from_millis(300))).unwrap();
            let _ = stream.read(&mut [0; 1024]);
            let _ = stream.write_all(b"HTTP/1.0 200 OK\r\nContent-Length: 2\r\n\r\nOK");
        }
        return;
    }
    if args.first().is_some_and(|s| s == "--branch") {
        fs::write("branch.pid", pid.to_string()).unwrap();
        let _child = Command::new(env::current_exe().unwrap()).arg("--leaf").spawn().unwrap();
        pause();
    }
    let executable = env::current_exe().unwrap();
    let name = executable.file_stem().unwrap().to_str().unwrap();
    let step = match name {
        "cargo" if args.first().is_some_and(|s| s == "check") => "wasm-check",
        "cargo" if args.iter().any(|s| s == "rssr-app") => "app-tests",
        "cargo" if args.iter().any(|s| s == "rssr-infra") => "host-contracts",
        "cargo" if args.first().is_some_and(|s| s == "run") => "cargo-run",
        "cargo" => "web-tests",
        "dx" => "web-bundle",
        _ => match args
            .iter()
            .find(|s| s.ends_with(".sh"))
            .map(|s| Path::new(s).file_name().unwrap().to_str().unwrap())
            .unwrap_or("")
        {
            "run_wasm_contract_harness.sh" => "browser-contracts",
            "run_rssr_web_auth_smoke.sh" => "web-auth",
            "run_static_web_reader_theme_matrix.sh" => "reader-theme-matrix",
            "run_static_web_small_viewport_smoke.sh" => "small-viewport",
            "run_rssr_web_proxy_feed_smoke.sh" => "proxy-feed",
            "run_rssr_web_browser_feed_smoke.sh" => "browser-feed",
            "run_web_spa_regression_server.sh" => "spa",
            _ => panic!("unexpected arguments: {args:?}"),
        },
    };
    writeln!(
        OpenOptions::new().create(true).append(true).open("trace.txt").unwrap(),
        "{step}\t{args:?}"
    )
    .unwrap();
    println!("fixture stdout: {step}");
    eprintln!("fixture stderr: {step}");
    if env::var("HOLD_STAGE").ok().as_deref() == Some(step) {
        let _child =
            Command::new(&executable).arg("--branch").stdin(Stdio::null()).spawn().unwrap();
        for _ in 0..200 {
            if Path::new("leaf.pid").is_file() {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        assert!(Path::new("leaf.pid").is_file());
        fs::write("tree-ready", "ready").unwrap();
        if env::var_os("ORPHAN").is_none() {
            pause();
        }
    }
    if env::var("FAIL_STAGE").ok().as_deref() == Some(step) {
        std::process::exit(23);
    }
    if step == "web-bundle" && env::var_os("MISSING_BUNDLE").is_none() {
        let profile = if args.iter().any(|s| s == "--release") { "release" } else { "debug" };
        fs::create_dir_all(format!("target/dx/rssr-app/{profile}/web/public")).unwrap();
    }
}
