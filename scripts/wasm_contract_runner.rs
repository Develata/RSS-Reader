//! Cargo owns artifact selection; this adapter only isolates browser test execution.
//! Compile with rustc, without adding an acceptance dependency to a product crate.

use std::{
    env,
    ffi::OsString,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    process::{Command, ExitCode, ExitStatus},
    time::{SystemTime, UNIX_EPOCH},
};

fn main() -> ExitCode {
    let arguments: Vec<_> = env::args_os().skip(1).collect();
    let result = if arguments.first().is_some_and(|argument| argument == "--artifact") {
        run_artifact(&arguments[1..], Path::new("wasm-bindgen-test-runner"))
    } else {
        run_harnesses(&arguments)
    };
    match result {
        Ok(status) => ExitCode::from(status.code().unwrap_or(1) as u8),
        Err(error) => {
            eprintln!("wasm contract runner: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_harnesses(harnesses: &[OsString]) -> io::Result<ExitStatus> {
    let executable = env::current_exe()?;
    cargo_command(harnesses, &executable)?.status()
}

fn cargo_command(harnesses: &[OsString], executable: &Path) -> io::Result<Command> {
    if harnesses.is_empty()
        || harnesses.iter().any(|harness| {
            harness.to_str().is_none_or(|name| {
                name.is_empty()
                    || !name
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(&byte))
            })
        })
    {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "expected harness name(s)"));
    }
    let configuration = format!(
        "target.wasm32-unknown-unknown.runner = [{}, \"--artifact\"]",
        json_string(path_text(executable)?)
    );
    let mut command = Command::new("cargo");
    command.args([
        "test",
        "--locked",
        "-p",
        "rssr-infra",
        "--target",
        "wasm32-unknown-unknown",
        "--config",
        &configuration,
    ]);
    for harness in harnesses {
        command.arg("--test").arg(harness);
    }
    Ok(command)
}

fn run_artifact(arguments: &[OsString], program: &Path) -> io::Result<ExitStatus> {
    if arguments.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Cargo did not provide a wasm file",
        ));
    }
    let temporary = TemporaryDirectory::create()?;
    let profile = temporary.0.join("chrome-profile");
    fs::create_dir(&profile)?;
    let configuration = temporary.0.join("webdriver.json");
    let chrome = find_program("google-chrome");
    fs::write(&configuration, webdriver_json(&profile, chrome.as_deref())?)?;

    let mut command = Command::new(program);
    command.args(arguments).env("WASM_BINDGEN_TEST_WEBDRIVER_JSON", configuration);
    // wasm-bindgen owns browser/driver teardown. Retain explicitly supplied timeouts.
    for (name, default) in
        [("WASM_BINDGEN_TEST_TIMEOUT", "60"), ("WASM_BINDGEN_TEST_DRIVER_TIMEOUT", "15")]
    {
        if env::var_os(name).is_none() {
            command.env(name, default);
        }
    }
    if chrome.is_some() && env::var_os("BROWSER").is_none() {
        command.env("BROWSER", "google-chrome");
    }
    command.status()
}

fn find_program(name: &str) -> Option<PathBuf> {
    env::split_paths(&env::var_os("PATH")?).find_map(|directory| {
        let path = directory.join(name);
        path.is_file().then(|| path.canonicalize().ok()).flatten()
    })
}

fn webdriver_json(profile: &Path, chrome: Option<&Path>) -> io::Result<String> {
    let mut fields = Vec::new();
    if let Some(chrome) = chrome {
        fields.push(format!("\"binary\":{}", json_string(path_text(chrome)?)));
    }
    let profile_argument = format!("--user-data-dir={}", path_text(profile)?);
    let arguments = [
        "--headless=new",
        "--no-sandbox",
        "--disable-dev-shm-usage",
        "--disable-gpu",
        "--remote-allow-origins=*",
        "--window-size=1280,720",
        &profile_argument,
    ]
    .map(json_string)
    .join(",");
    fields.push(format!("\"args\":[{arguments}]"));
    Ok(format!("{{\"goog:chromeOptions\":{{{}}}}}", fields.join(",")))
}

fn path_text(path: &Path) -> io::Result<&str> {
    path.to_str()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path is not valid UTF-8"))
}

// JSON strings also form valid TOML basic strings, including control-character escapes.
fn json_string(text: &str) -> String {
    let mut result = String::with_capacity(text.len() + 2);
    result.push('"');
    for character in text.chars() {
        match character {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\u{0}'..='\u{1f}' | '\u{7f}' => {
                use std::fmt::Write;
                write!(result, "\\u{:04x}", character as u32)
                    .expect("writing to a String cannot fail");
            }
            character => result.push(character),
        }
    }
    result.push('"');
    result
}

struct TemporaryDirectory(PathBuf);

impl TemporaryDirectory {
    fn create() -> io::Result<Self> {
        let root = env::temp_dir().canonicalize()?;
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
        for attempt in 0..128 {
            let path = root.join(format!("rssr-wasm-{}-{stamp}-{attempt}", std::process::id()));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(io::ErrorKind::AlreadyExists, "could not reserve temporary directory"))
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            let _ = writeln!(io::stderr(), "could not remove {}: {error}", self.0.display());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    #[test]
    fn strings_escape_paths_and_every_control_character() {
        assert_eq!(
            json_string("中文 空格/\"a\\b\n\r\t"),
            "\"中文 空格/\\\"a\\\\b\\u000a\\u000d\\u0009\""
        );
        for character in ('\u{0}'..='\u{1f}').chain(['\u{7f}']) {
            assert_eq!(
                json_string(&character.to_string()),
                format!("\"\\u{:04x}\"", character as u32)
            );
        }
    }

    #[test]
    fn cargo_selects_multiple_targets_and_receives_an_unambiguous_runner_array() {
        let command =
            cargo_command(&["first".into(), "second".into()], Path::new("/中文 空格/\"runner"))
                .unwrap();
        let arguments: Vec<_> = command.get_args().collect();
        assert_eq!(
            &arguments[..7],
            [
                "test",
                "--locked",
                "-p",
                "rssr-infra",
                "--target",
                "wasm32-unknown-unknown",
                "--config"
            ]
            .map(OsStr::new)
        );
        assert_eq!(
            arguments[7],
            "target.wasm32-unknown-unknown.runner = [\"/中文 空格/\\\"runner\", \"--artifact\"]"
        );
        assert_eq!(&arguments[8..], ["--test", "first", "--test", "second"].map(OsStr::new));
        for invalid in [vec![], vec!["--artifact".into(), "".into()], vec!["../bad".into()]] {
            assert!(cargo_command(&invalid, Path::new("runner")).is_err());
        }
    }

    #[test]
    fn capabilities_escape_browser_and_profile_paths_without_a_shared_file() {
        let profile = Path::new("/中文 profile/\"quote\\backslash");
        let configuration = webdriver_json(profile, Some(Path::new("/浏览器 \"chrome"))).unwrap();
        assert!(configuration.contains("\"binary\":\"/浏览器 \\\"chrome\""));
        assert!(configuration.contains("\"--user-data-dir=/中文 profile/\\\"quote\\\\backslash\""));
        assert!(!webdriver_json(profile, None).unwrap().contains("\"binary\""));
    }

    #[test]
    fn temporary_directories_are_unique_and_removed_on_drop() {
        let first = TemporaryDirectory::create().unwrap();
        let second = TemporaryDirectory::create().unwrap();
        assert_ne!(first.0, second.0);
        let path = first.0.clone();
        fs::write(path.join("partial.json"), "partial").unwrap();
        drop(first);
        assert!(!path.exists());
        assert!(second.0.exists());
    }

    #[cfg(unix)]
    #[test]
    fn concurrent_processes_forward_arguments_and_clean_up_after_failure() {
        use std::os::unix::fs::PermissionsExt;

        let fixture = TemporaryDirectory::create().unwrap();
        let runners = [fixture.0.join("第一 runner\""), fixture.0.join("第二 runner")];
        for runner in &runners {
            fs::write(
                runner,
                r#"#!/bin/sh
printf '%s\0' "$@" > "${0}.args"
printf '%s' "$WASM_BINDGEN_TEST_WEBDRIVER_JSON" > "${0}.path"
cp "$WASM_BINDGEN_TEST_WEBDRIVER_JSON" "${0}.json"
# Overlap the two processes, exposing accidentally shared configuration/profile paths.
sleep 0.05
exit 7
"#,
            )
            .unwrap();
            fs::set_permissions(runner, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let arguments =
            [OsString::from("/中文 空格/\"target.wasm"), "filter name".into(), "--exact".into()];
        std::thread::scope(|scope| {
            let children: Vec<_> = runners
                .iter()
                .map(|runner| {
                    scope.spawn(|| run_artifact(&arguments, runner).unwrap().code().unwrap())
                })
                .collect();
            for child in children {
                assert_eq!(child.join().unwrap(), 7);
            }
        });
        let mut paths = Vec::new();
        for runner in &runners {
            let report = |suffix: &str| {
                let mut path = runner.as_os_str().to_owned();
                path.push(suffix);
                PathBuf::from(path)
            };
            assert_eq!(
                fs::read(report(".args")).unwrap(),
                "/中文 空格/\"target.wasm\0filter name\0--exact\0".as_bytes()
            );
            let path = PathBuf::from(fs::read_to_string(report(".path")).unwrap());
            let configuration = fs::read_to_string(report(".json")).unwrap();
            assert!(configuration.contains(&json_string(&format!(
                "--user-data-dir={}",
                path.parent().unwrap().join("chrome-profile").display()
            ))));
            assert!(!path.parent().unwrap().exists());
            paths.push(path);
        }
        assert_ne!(paths[0], paths[1]);
    }
}
