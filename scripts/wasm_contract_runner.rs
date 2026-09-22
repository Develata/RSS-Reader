//! Cargo owns artifact selection; this adapter collects or isolates browser test execution.
//! Compile with rustc, without adding an acceptance dependency to a product crate.

use std::{
    env,
    ffi::OsString,
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::{Command, ExitCode, ExitStatus},
    time::{SystemTime, UNIX_EPOCH},
};

fn main() -> ExitCode {
    let arguments: Vec<_> = env::args_os().skip(1).collect();
    let result = dispatch(&arguments);
    match result {
        Ok(code) => code,
        Err(error) => {
            eprintln!("wasm contract runner: {error}");
            ExitCode::FAILURE
        }
    }
}

fn exit_code(status: ExitStatus) -> ExitCode {
    ExitCode::from(status.code().unwrap_or(1) as u8)
}

fn dispatch(arguments: &[OsString]) -> io::Result<ExitCode> {
    let browser_runner = Path::new("wasm-bindgen-test-runner");
    match arguments.first().and_then(|argument| argument.to_str()) {
        Some("--artifact") => run_artifact(&arguments[1..], browser_runner).map(exit_code),
        Some("--prepare") if arguments.len() >= 3 => {
            let executable = env::current_exe()?;
            prepare_bundle(Path::new(&arguments[1]), &arguments[2..], |directory| {
                cargo_command(
                    &arguments[2..],
                    &[executable.into(), "--collect".into(), directory.into()],
                )?
                .status()
            })
            .map(exit_code)
        }
        Some("--collect") if arguments.len() == 3 => {
            collect_artifact(Path::new(&arguments[1]), Path::new(&arguments[2]))?;
            Ok(ExitCode::SUCCESS)
        }
        Some("--prebuilt") if arguments.len() == 3 => {
            let artifact = prebuilt_artifact(Path::new(&arguments[1]), &arguments[2])?;
            run_artifact(&[artifact.into()], browser_runner).map(exit_code)
        }
        Some(argument) if argument.starts_with("--") => Err(invalid_input(
            "expected --prepare DIR HARNESS..., --prebuilt DIR HARNESS, or HARNESS...",
        )),
        _ => run_harnesses(arguments).map(exit_code),
    }
}

fn run_harnesses(harnesses: &[OsString]) -> io::Result<ExitStatus> {
    let executable = env::current_exe()?;
    cargo_command(harnesses, &[executable.into(), "--artifact".into()])?.status()
}

fn invalid_input(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}

fn harness_names(harnesses: &[OsString]) -> io::Result<Vec<&str>> {
    let mut names = Vec::with_capacity(harnesses.len());
    for harness in harnesses {
        let name = harness.to_str().ok_or_else(|| invalid_input("harness is not valid UTF-8"))?;
        if name.is_empty()
            || !name.bytes().all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(&byte))
            || name.starts_with('-')
            || names.contains(&name)
        {
            return Err(invalid_input("expected distinct harness names"));
        }
        names.push(name);
    }
    if names.is_empty() {
        return Err(invalid_input("expected harness name(s)"));
    }
    Ok(names)
}

fn cargo_command(harnesses: &[OsString], runner: &[OsString]) -> io::Result<Command> {
    harness_names(harnesses)?;
    let runner = runner
        .iter()
        .map(|argument| {
            argument
                .to_str()
                .map(json_string)
                .ok_or_else(|| invalid_input("runner argument is not valid UTF-8"))
        })
        .collect::<io::Result<Vec<_>>>()?;
    let configuration = format!("target.wasm32-unknown-unknown.runner = [{}]", runner.join(", "));
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

const PENDING_MANIFEST: &str = "pending-harnesses.txt";
const MANIFEST: &str = "harnesses.txt";

fn prepare_bundle(
    directory: &Path,
    harnesses: &[OsString],
    collect: impl FnOnce(&Path) -> io::Result<ExitStatus>,
) -> io::Result<ExitStatus> {
    let names = harness_names(harnesses)?;
    if let Some(parent) = directory.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    // create_dir, rather than create_dir_all, rejects every pre-existing destination.
    // Only a fully collected bundle gains its manifest, so --prebuilt cannot run it early.
    fs::create_dir(directory)?;
    let mut prepared = TemporaryDirectory(directory.to_owned());
    let directory = directory.canonicalize()?;
    fs::write(directory.join(PENDING_MANIFEST), names.join("\n") + "\n")?;
    let status = collect(&directory)?;
    if status.success() {
        validate_bundle(&directory, PENDING_MANIFEST)?;
        fs::rename(directory.join(PENDING_MANIFEST), directory.join(MANIFEST))?;
        prepared.0.clear(); // Keep only the complete bundle; Drop cleans every failure path.
    }
    Ok(status)
}

fn manifest_names(directory: &Path, manifest: &str) -> io::Result<Vec<String>> {
    let text = fs::read_to_string(directory.join(manifest))?;
    let names: Vec<OsString> = text.lines().map(OsString::from).collect();
    Ok(harness_names(&names)?.into_iter().map(str::to_owned).collect())
}

fn validate_wasm(path: &Path) -> io::Result<()> {
    let mut header = [0; 8];
    fs::File::open(path)?.read_exact(&mut header)?;
    if header != *b"\0asm\x01\0\0\0" {
        return Err(invalid_input(format!("not a WebAssembly module: {}", path.display())));
    }
    Ok(())
}

fn collect_artifact(directory: &Path, artifact: &Path) -> io::Result<()> {
    let names = manifest_names(directory, PENDING_MANIFEST)?;
    let stem = artifact
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or_else(|| invalid_input("Cargo artifact has no UTF-8 file stem"))?;
    let (name, hash) = stem
        .rsplit_once('-')
        .ok_or_else(|| invalid_input("Cargo artifact must include its build hash"))?;
    if artifact.extension().is_none_or(|extension| extension != "wasm")
        || hash.is_empty()
        || !hash.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(invalid_input("Cargo supplied an unexpected harness artifact"));
    }
    // rustc normalizes target-name hyphens in artifact filenames. Require one unique match.
    let mut matches = names.iter().filter(|expected| expected.replace('-', "_") == name);
    let name =
        matches.next().ok_or_else(|| invalid_input("Cargo supplied an unrequested harness"))?;
    if matches.next().is_some() {
        return Err(invalid_input("harness names map to the same Cargo artifact"));
    }
    validate_wasm(artifact)?;
    // create_new ensures repeated target execution cannot silently replace an artifact.
    let mut target = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(directory.join(format!("{name}.wasm")))?;
    io::copy(&mut fs::File::open(artifact)?, &mut target)?;
    Ok(())
}

fn validate_bundle(directory: &Path, manifest: &str) -> io::Result<Vec<String>> {
    let names = manifest_names(directory, manifest)?;
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let name = entry.file_name();
        let allowed = name == manifest
            || names.iter().any(|expected| name == format!("{expected}.wasm").as_str());
        if !allowed || !entry.file_type()?.is_file() {
            return Err(invalid_input(format!(
                "unexpected bundle entry: {}",
                entry.path().display()
            )));
        }
    }
    for name in &names {
        validate_wasm(&directory.join(format!("{name}.wasm")))?;
    }
    Ok(names)
}

fn prebuilt_artifact(directory: &Path, harness: &OsString) -> io::Result<PathBuf> {
    let requested = harness_names(std::slice::from_ref(harness))?[0];
    let names = validate_bundle(directory, MANIFEST)?;
    if !names.iter().any(|name| name == requested) {
        return Err(invalid_input("requested harness is absent from the bundle manifest"));
    }
    directory.join(format!("{requested}.wasm")).canonicalize()
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
        if self.0.as_os_str().is_empty() {
            return;
        }
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
        let runner = ["/中文 空格/\"runner".into(), "--artifact".into()];
        let command = cargo_command(&["first".into(), "second".into()], &runner).unwrap();
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
        for invalid in [
            vec![],
            vec!["--artifact".into(), "".into()],
            vec!["../bad".into()],
            vec!["same".into(), "same".into()],
        ] {
            assert!(cargo_command(&invalid, &runner).is_err());
        }
    }

    fn write_wasm(path: &Path) {
        fs::write(path, b"\0asm\x01\0\0\0").unwrap();
    }

    #[cfg(unix)]
    fn process_status(code: u8) -> ExitStatus {
        Command::new("sh").args(["-c", &format!("exit {code}")]).status().unwrap()
    }

    #[cfg(unix)]
    #[test]
    fn prepare_selects_cargo_artifacts_and_publishes_only_the_complete_bundle() {
        let fixture = TemporaryDirectory::create().unwrap();
        let source = fixture.0.join("中文 输入 with spaces");
        fs::create_dir(&source).unwrap();
        let destination = fixture.0.join("fresh target/中文 产物 \" bundle");
        let harnesses = ["first".into(), "second-harness".into()];
        for name in ["first-abcd.wasm", "second_harness-1234.wasm"] {
            write_wasm(&source.join(name));
        }
        // A stale target beside the actual Cargo paths must never be selected by a glob.
        fs::write(source.join("first-9999.wasm"), "stale").unwrap();
        let status = prepare_bundle(&destination, &harnesses, |directory| {
            collect_artifact(directory, &source.join("first-abcd.wasm"))?;
            assert!(prebuilt_artifact(directory, &"first".into()).is_err());
            collect_artifact(directory, &source.join("second_harness-1234.wasm"))?;
            Ok(process_status(0))
        })
        .unwrap();
        assert!(status.success());
        assert!(!destination.join(PENDING_MANIFEST).exists());
        for name in ["first", "second-harness"] {
            let artifact = prebuilt_artifact(&destination, &name.into()).unwrap();
            assert_eq!(artifact, destination.join(format!("{name}.wasm")));
            assert_eq!(fs::read(artifact).unwrap(), b"\0asm\x01\0\0\0");
        }
        assert!(prebuilt_artifact(&destination, &"unknown".into()).is_err());
        assert!(prebuilt_artifact(&destination, &"../first".into()).is_err());
        let manifest = fs::read(destination.join(MANIFEST)).unwrap();
        assert!(
            prepare_bundle(&destination, &harnesses, |_| panic!("must not run Cargo")).is_err()
        );
        assert_eq!(fs::read(destination.join(MANIFEST)).unwrap(), manifest);
    }

    #[cfg(unix)]
    #[test]
    fn prepare_cleans_partial_output_on_cargo_failure_incomplete_success_and_io_error() {
        let fixture = TemporaryDirectory::create().unwrap();
        let artifact = fixture.0.join("first-abcd.wasm");
        write_wasm(&artifact);
        for mode in ["cargo failure", "incomplete success", "io error"] {
            let destination = fixture.0.join(mode);
            let result =
                prepare_bundle(&destination, &["first".into(), "missing".into()], |directory| {
                    collect_artifact(directory, &artifact)?;
                    match mode {
                        "cargo failure" => Ok(process_status(7)),
                        "incomplete success" => Ok(process_status(0)),
                        _ => Err(io::Error::other("cannot start Cargo")),
                    }
                });
            if mode == "cargo failure" {
                assert_eq!(result.unwrap().code(), Some(7));
            } else {
                assert!(result.is_err());
            }
            assert!(!destination.exists(), "{mode} left a partial bundle");
        }
    }

    #[test]
    fn collection_rejects_unrequested_corrupt_and_repeated_targets() {
        let fixture = TemporaryDirectory::create().unwrap();
        let destination = fixture.0.join("bundle");
        fs::create_dir(&destination).unwrap();
        fs::write(destination.join(PENDING_MANIFEST), "first\n").unwrap();
        for name in ["unrequested-abcd.wasm", "first-nonhex.wasm", "first.wasm", "first-abcd.txt"] {
            let artifact = fixture.0.join(name);
            write_wasm(&artifact);
            assert!(collect_artifact(&destination, &artifact).is_err(), "accepted {name}");
        }
        let artifact = fixture.0.join("first-abcd.wasm");
        fs::write(&artifact, "not wasm").unwrap();
        assert!(collect_artifact(&destination, &artifact).is_err());
        assert!(!destination.join("first.wasm").exists());
        write_wasm(&artifact);
        collect_artifact(&destination, &artifact).unwrap();
        assert!(collect_artifact(&destination, &artifact).is_err());
        fs::write(destination.join(PENDING_MANIFEST), "first-other\nfirst_other\n").unwrap();
        let ambiguous = fixture.0.join("first_other-abcd.wasm");
        write_wasm(&ambiguous);
        assert!(collect_artifact(&destination, &ambiguous).is_err());
    }

    #[test]
    fn prebuilt_rejects_missing_corrupt_extra_files_and_duplicate_manifest_entries() {
        let fixture = TemporaryDirectory::create().unwrap();
        fs::write(fixture.0.join(MANIFEST), "first\n").unwrap();
        let name = OsString::from("first");
        assert!(prebuilt_artifact(&fixture.0, &name).is_err());
        let artifact = fixture.0.join("first.wasm");
        fs::write(&artifact, "bad").unwrap();
        assert!(prebuilt_artifact(&fixture.0, &name).is_err());
        write_wasm(&artifact);
        assert!(prebuilt_artifact(&fixture.0, &name).is_ok());
        let stale = fixture.0.join("first-stale.wasm");
        write_wasm(&stale);
        assert!(prebuilt_artifact(&fixture.0, &name).is_err());
        fs::remove_file(stale).unwrap();
        for manifest in ["", "first\nfirst\n", "../first\n", "first\n\n", "first\nmissing\n"] {
            fs::write(fixture.0.join(MANIFEST), manifest).unwrap();
            assert!(prebuilt_artifact(&fixture.0, &name).is_err(), "accepted {manifest:?}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn prebuilt_rejects_symlinks_and_prepare_preserves_existing_directories() {
        use std::os::unix::fs::symlink;

        let fixture = TemporaryDirectory::create().unwrap();
        let destination = fixture.0.join("bundle");
        fs::create_dir(&destination).unwrap();
        let artifact = fixture.0.join("source.wasm");
        write_wasm(&artifact);
        fs::write(destination.join(MANIFEST), "first\n").unwrap();
        symlink(&artifact, destination.join("first.wasm")).unwrap();
        assert!(prebuilt_artifact(&destination, &"first".into()).is_err());
        assert!(
            prepare_bundle(&destination, &["first".into()], |_| panic!("must not run")).is_err()
        );
        assert!(destination.join("first.wasm").exists());
        let alias = fixture.0.join("alias");
        symlink(&destination, &alias).unwrap();
        assert!(prepare_bundle(&alias, &["first".into()], |_| panic!("must not run")).is_err());
        assert!(destination.join("first.wasm").exists());
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
