"""Black-box release CLI tests. Uses only Python stdlib plus installed rustc/Bash.

Run after cargo build; optional args: --binary PATH --bash PATH.
Artifacts remain under target/release-ui-acceptance for diagnosis.
"""
import argparse
import ctypes
import json
import os
from pathlib import Path
import shutil
import shlex
import signal
import socket
import subprocess
import sys
import time
import unittest

ROOT = Path(__file__).resolve().parents[3]
WINDOWS = os.name == "nt"
SUFFIX = ".exe" if WINDOWS else ""
parser = argparse.ArgumentParser()
parser.add_argument("--binary", type=Path, default=ROOT / "target/release-ui-runner/debug" / ("release-ui" + SUFFIX))
parser.add_argument("--bash", default=os.environ.get("RSSR_BASH", "bash"))
ARGS, remaining = parser.parse_known_args()
BINARY = ARGS.binary.resolve()
RUN = ROOT / "target/release-ui-acceptance" / str(time.time_ns())
RUN.mkdir(parents=True)
FIXTURE = RUN / ("fixture" + SUFFIX)
subprocess.run(["rustc", "--edition=2024", str(Path(__file__).with_name("fixture.rs")), "-o", str(FIXTURE)], check=True)


def free_port():
    with socket.socket() as probe:
        probe.bind(("127.0.0.1", 0))
        return probe.getsockname()[1]


def alive(pid):
    if WINDOWS:
        kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        kernel.OpenProcess.restype = ctypes.c_void_p
        handle = kernel.OpenProcess(0x1000, False, pid)
        if not handle:
            return False
        code = ctypes.c_ulong()
        kernel.GetExitCodeProcess(ctypes.c_void_p(handle), ctypes.byref(code))
        kernel.CloseHandle(ctypes.c_void_p(handle))
        return code.value == 259
    try:
        # Killed descendants may remain zombies until the container init reaps.
        stat = Path(f"/proc/{pid}/stat")
        if stat.exists() and stat.read_text().split(") ")[1].startswith("Z"):
            return False
        os.kill(pid, 0)
        return True
    except ProcessLookupError:
        return False


def console_event(pid, event):
    # Attach a helper to THIS test process's hidden private console only. Never
    # broadcast into the user's console or another application's process group.
    code = """
import ctypes, sys, time
k = ctypes.WinDLL('kernel32', use_last_error=True)
k.FreeConsole()
assert k.AttachConsole(int(sys.argv[1])), ctypes.get_last_error()
handler = ctypes.WINFUNCTYPE(ctypes.c_int, ctypes.c_ulong)(lambda _: 1)
assert k.SetConsoleCtrlHandler(handler, True), ctypes.get_last_error()
assert k.GenerateConsoleCtrlEvent(int(sys.argv[2]), 0), ctypes.get_last_error()
time.sleep(.2)
k.FreeConsole()
"""
    subprocess.run([sys.executable, "-c", code, str(pid), str(event)], check=True, timeout=5)


class Acceptance(unittest.TestCase):
    def setUp(self):
        self.root = RUN / (self._testMethodName + " 中文 space")
        self.root.mkdir()
        (self.root / "scripts").mkdir()
        (self.root / "scripts/run_web_spa_regression_server.sh").touch()
        tools = self.root / "tools"
        tools.mkdir()
        for name in ["cargo", "dx", "bash"]:
            shutil.copy2(FIXTURE, tools / (name + SUFFIX))
        self.env = dict(os.environ, PATH=str(tools) + os.pathsep + os.environ["PATH"], RSSR_BASH=str(tools / ("bash" + SUFFIX)))
        self.log = self.root / "结果 logs"
        self.processes = []

    def tearDown(self):
        for proc in self.processes:
            if proc.poll() is None:
                proc.kill()
                proc.wait(timeout=8)
        # Also clean known fixture descendants when testing a regressed runner.
        # Assertions run before this safety net and must prove runner-owned cleanup.
        for name in ["branch.pid", "leaf.pid"]:
            path = self.root / name
            if path.exists():
                pid = int(path.read_text())
                if alive(pid):
                    os.kill(pid, signal.SIGTERM if WINDOWS else signal.SIGKILL)

    def command(self, *args):
        return [str(BINARY), "--repo-root", str(self.root), "--log-dir", str(self.log), *args]

    def run_cli(self, *args, code=0):
        result = subprocess.run(self.command(*args), env=self.env, cwd=RUN, capture_output=True, timeout=25)
        self.assertEqual(result.returncode, code, result.stdout.decode(errors="replace") + result.stderr.decode(errors="replace"))
        return result

    def report(self):
        return json.loads((self.log / "summary.json").read_text(encoding="utf-8"))

    def stage(self, name):
        return next(s for s in self.report()["steps"] if s["name"] == name)

    def bundle(self):
        (self.root / "target/dx/rssr-app/debug/web/public").mkdir(parents=True)

    def start(self, *args, unread=False):
        kwargs = {}
        if WINDOWS:
            info = subprocess.STARTUPINFO()
            info.dwFlags |= subprocess.STARTF_USESHOWWINDOW
            info.wShowWindow = 0
            kwargs.update(creationflags=subprocess.CREATE_NEW_CONSOLE, startupinfo=info)
        output = open(self.root / "console.log", "wb")
        proc = subprocess.Popen(self.command(*args), env=self.env, cwd=RUN, stdout=subprocess.PIPE if unread else output, stderr=subprocess.STDOUT, **kwargs)
        output.close()
        self.processes.append(proc)
        if unread:
            self.addCleanup(proc.stdout.close)
        return proc

    def until(self, predicate, proc=None):
        deadline = time.monotonic() + 12
        while time.monotonic() < deadline:
            try:
                if predicate():
                    return
            except (FileNotFoundError, json.JSONDecodeError):
                pass
            if proc is not None and proc.poll() is not None:
                self.fail((self.root / "console.log").read_text(errors="replace"))
            time.sleep(.04)
        self.fail("condition timed out")

    def no_tree(self):
        pids = [int((self.root / name).read_text()) for name in ["branch.pid", "leaf.pid"]]
        self.until(lambda: all(not alive(pid) for pid in pids))
        self.assert_port_free(int(self.env["FIXTURE_PORT"]))

    def assert_port_free(self, port):
        with socket.socket() as probe:
            probe.settimeout(.2)
            self.assertNotEqual(probe.connect_ex(("127.0.0.1", port)), 0, "listener still reachable")
        with socket.socket() as probe:
            # A completed HTTP probe can leave TIME_WAIT after every PID exited.
            # On Unix reuse that state, never a live listener (no SO_REUSEPORT).
            # Windows SO_REUSEADDR has different semantics; keep its strict bind.
            if not WINDOWS:
                probe.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            probe.bind(("127.0.0.1", port))

    def test_parameters_and_plan_have_no_side_effects(self):
        for args in [["--port"], ["--web-port"], ["--log-dir"], ["--what"], ["--port", "0"], ["--port", "65536"], ["--full", "--port", "55525"]]:
            self.run_cli(*args, code=1)
        self.run_cli("--help")
        plan = json.loads(self.run_cli("--plan", "--full", "--release", "--debug").stdout)
        self.assertEqual(sum(s["name"] == "web-bundle" and s["enabled"] for s in plan), 1)
        self.assertFalse(self.log.exists())
        self.assertFalse((self.root / "trace.txt").exists())

    def test_bash_stage_preserves_caller_tools(self):
        self.env["RSSR_BASH"] = ARGS.bash
        shutil.copy2(FIXTURE, self.root / "tools" / ("uname" + SUFFIX))
        if WINDOWS:
            # Seed PATH in Bash itself: MSYS may import a cached native PATH.
            # Unset the hook so a nested interpreter cannot undo a bad prepend.
            startup = self.root / "caller-bash-env"
            startup.write_text('unset BASH_ENV\nexport PATH="$(/usr/bin/cygpath -u "$FIXTURE_TOOLS"):$PATH"\n', encoding="utf-8")
            self.env.update(BASH_ENV=startup.as_posix(), FIXTURE_TOOLS=str(self.root / "tools"))
        (self.root / "scripts/run_wasm_contract_harness.sh").write_text(
            'set -eu\nuname > selected-tool.txt\n', encoding="utf-8")
        self.run_cli("--skip-automated", "--with-browser-contracts", "--no-serve")
        self.assertEqual((self.root / "selected-tool.txt").read_text().strip(), "caller-selected-uname")

    @unittest.skipIf(WINDOWS, "alternate Bash executable identity is checked on Unix")
    def test_explicit_bash_is_not_replaced(self):
        chosen = self.root / "selected-bash"
        chosen.symlink_to(Path(shutil.which(ARGS.bash) or ARGS.bash).resolve())
        self.env["RSSR_BASH"] = str(chosen)
        (self.root / "scripts/run_wasm_contract_harness.sh").write_text(
            'printf "%s" "$BASH" > selected-shell.txt\n', encoding="utf-8")
        self.run_cli("--skip-automated", "--with-browser-contracts", "--no-serve")
        self.assertEqual((self.root / "selected-shell.txt").read_text(), str(chosen))

    def test_unread_stdout_does_not_block_cancellation(self):
        self.env.update(HOLD_STAGE="wasm-check", FIXTURE_PORT=str(free_port()), NOISY="1")
        proc = self.start("--full", "--no-serve", unread=True)
        self.until(lambda: (self.root / "noise-ready").exists(), proc)
        time.sleep(.2)  # Give the supervisor time to fill the unread stdout pipe.
        started = time.monotonic()
        if WINDOWS:
            console_event(proc.pid, 1)
        else:
            proc.send_signal(signal.SIGTERM)
        self.assertEqual(proc.wait(timeout=5), 130 if WINDOWS else 143)
        self.assertLess(time.monotonic() - started, 5)
        self.assertEqual(self.stage("wasm-check")["status"], "interrupted")
        self.assertEqual(self.stage("app-tests")["status"], "blocked")
        self.assertGreater((self.log / "automated-gates.log").stat().st_size, 4 * 1024 * 1024)
        self.no_tree()

    def test_git_revision_in_linked_worktree_and_separate_gitdir(self):
        def git(*args):
            return subprocess.check_output(["git", "-C", str(self.root), *args], stderr=subprocess.STDOUT).decode().strip()
        git("init", "-q")
        git("add", "scripts")
        git("-c", "user.name=CLI fixture", "-c", "user.email=fixture@example.invalid", "commit", "-qm", "fixture")
        expected = git("rev-parse", "HEAD")
        git("pack-refs", "--all")
        self.run_cli("--skip-automated", "--no-serve")
        self.assertEqual(self.report()["commit"], expected)
        linked = self.root / "linked 中文 worktree"
        git("worktree", "add", "--detach", str(linked), "HEAD")
        self.assertTrue((linked / ".git").is_file())
        self.run_cli("--repo-root", str(linked), "--skip-automated", "--no-serve")
        self.assertEqual(self.report()["commit"], expected)
        git("worktree", "remove", str(linked))
        git("init", "--separate-git-dir", str(self.root / "git storage"))
        self.assertTrue((self.root / ".git").is_file())
        self.run_cli("--skip-automated", "--no-serve")
        self.assertEqual(self.report()["commit"], expected)

    def test_git_probe_cancellation_owns_descendants(self):
        (self.root / ".git").touch()
        shutil.copy2(FIXTURE, self.root / "tools" / ("git" + SUFFIX))
        self.env.update(HOLD_STAGE="git-revision", FIXTURE_PORT=str(free_port()))
        proc = self.start("--no-serve")
        self.until(lambda: (self.root / "tree-ready").exists(), proc)
        if WINDOWS:
            console_event(proc.pid, 0)
        else:
            proc.send_signal(signal.SIGTERM)
        self.assertEqual(proc.wait(timeout=5), 130 if WINDOWS else 143)
        self.assertEqual(self.report()["outcome"], "interrupted")
        self.no_tree()

    def test_real_launcher_target_config_and_stale_binary(self):
        # Real cargo builds the real CLI through the real shell entry point.
        # The fixture repository lives under this checkout's ignored target, not C:.
        shutil.copy2(ROOT / "scripts/run_release_ui_regression.sh", self.root / "scripts/run_release_ui_regression.sh")
        shutil.copytree(ROOT / "scripts/release-ui", self.root / "scripts/release-ui", ignore=shutil.ignore_patterns("target", "__pycache__"))
        env = dict(os.environ, CARGO_NET_OFFLINE="true")
        host = next(line.split(": ", 1)[1] for line in subprocess.check_output(["rustc", "-vV"]).decode().splitlines() if line.startswith("host: "))
        target = self.root / "target/release-ui-runner"
        stale = target / "debug" / ("release-ui" + SUFFIX)
        stale.parent.mkdir(parents=True)
        shutil.copy2(FIXTURE, stale)  # Any accidental execution cannot print a JSON plan.
        config = self.root / ".cargo/config.toml"
        config.parent.mkdir()
        cargo_wrapper = self.root / "caller tools"
        cargo_wrapper.mkdir()
        marker = self.root / "caller-cargo.txt"
        real_cargo = Path(shutil.which("cargo")).as_posix()
        wrapper = cargo_wrapper / "cargo"
        wrapper.write_text(f'#!/bin/bash\nprintf "selected\\n" >> {shlex.quote(marker.as_posix())}\nexec {shlex.quote(real_cargo)} "$@"\n', encoding="utf-8")
        wrapper.chmod(0o755)
        env["FIXTURE_TOOLS"] = str(cargo_wrapper)
        # Establish caller preference immediately before invoking the actual
        # launcher. Positional argv preserves spaces and avoids shell injection.
        invoke = 'tools="$1"; shift; if [[ "$OSTYPE" == msys* || "$OSTYPE" == cygwin* ]]; then tools="$(/usr/bin/cygpath -u "$tools")"; fi; export PATH="$tools:$PATH"; exec "$BASH" "$@"'
        for mode in ["environment", "config", "missing-unqualified"]:
            with self.subTest(mode=mode):
                if mode == "environment":
                    env["CARGO_BUILD_TARGET"] = host
                else:
                    env.pop("CARGO_BUILD_TARGET", None)
                    config.write_text(f'[build]\ntarget = "{host}"\n', encoding="utf-8")
                    if mode == "missing-unqualified":
                        stale.unlink(missing_ok=True)
                result = subprocess.run([ARGS.bash, "--noprofile", "--norc", "-c", invoke, "test-launcher", str(cargo_wrapper), str(self.root / "scripts/run_release_ui_regression.sh"), "--full", "--plan"], env=env, cwd=RUN, capture_output=True, timeout=180)
                (self.root / f"launcher-{mode}.log").write_bytes(result.stdout + result.stderr)
                self.assertEqual(result.returncode, 0, result.stderr.decode(errors="replace"))
                self.assertTrue(any(s["name"] == "web-bundle" for s in json.loads(result.stdout)))
        self.assertEqual(marker.read_text().splitlines(), ["selected"] * 3)

    def test_launcher_artifact_path_has_no_line_ending(self):
        # Exercise the actual embedded reader with native Python too; the
        # launcher's preferred python3 may be an MSYS build on Windows.
        launcher = (ROOT / "scripts/run_release_ui_regression.sh").read_text(encoding="utf-8")
        reader = launcher.split('| "$python" -X utf8 -c \'\n', 1)[1].split("\n')\"", 1)[0]
        executable = self.root / ("release-ui" + SUFFIX)
        artifact = {
            "reason": "compiler-artifact",
            "target": {"name": "release-ui", "kind": ["bin"]},
            "profile": {"test": False},
            "executable": str(executable),
        }
        result = subprocess.run([sys.executable, "-X", "utf8", "-c", reader], input=(json.dumps(artifact) + "\n").encode(), capture_output=True, timeout=15)
        self.assertEqual(result.returncode, 0, result.stderr.decode(errors="replace"))
        self.assertEqual(result.stdout, executable.as_posix().encode("utf-8"))

    def test_real_launcher_respects_configured_compiler(self):
        # A real compiler remains available by absolute path while a native
        # poison executable shadows bare rustc, including under Git Bash.
        launcher = self.root / "scripts/run_release_ui_regression.sh"
        shutil.copy2(ROOT / "scripts/run_release_ui_regression.sh", launcher)
        shutil.copytree(ROOT / "scripts/release-ui", self.root / "scripts/release-ui", ignore=shutil.ignore_patterns("target", "__pycache__"))
        sysroot = Path(subprocess.check_output(["rustc", "--print", "sysroot"]).decode().strip())
        compiler = sysroot / "bin" / ("rustc" + SUFFIX)
        self.assertTrue(compiler.is_file())
        poison = self.root / "poison tools"
        poison.mkdir()
        poison_rustc = poison / ("rustc" + SUFFIX)
        shutil.copy2(FIXTURE, poison_rustc)
        config = self.root / ".cargo/config.toml"
        config.parent.mkdir()
        stale = self.root / "target/release-ui-runner/debug" / ("release-ui" + SUFFIX)
        stale.parent.mkdir(parents=True)
        shutil.copy2(FIXTURE, stale)
        # This is the pre-fix launcher probe, retained only as an A/B fixture.
        old_probe = self.root / "scripts/old-rustc-probe.sh"
        old_probe.write_text(
            'set -euo pipefail\nhost=""\n'
            'while IFS=" " read -r key value; do\n'
            '  if [[ "$key" == host: ]]; then host="$value"; fi\n'
            'done <<< "$(rustc -vV)"\n'
            'if [[ -z "$host" ]]; then echo "Could not resolve rustc host target" >&2; exit 1; fi\n',
            encoding="utf-8")
        invoke = 'tools="$1"; shift; if [[ "$OSTYPE" == msys* || "$OSTYPE" == cygwin* ]]; then tools="$(/usr/bin/cygpath -u "$tools")"; fi; export PATH="$tools:$PATH"; exec "$BASH" "$@"'
        for mode in ["RUSTC", "CARGO_BUILD_RUSTC", "config"]:
            with self.subTest(mode=mode):
                env = dict(os.environ, CARGO_NET_OFFLINE="true", CARGO_BUILD_TARGET="wasm32-unknown-unknown")
                env.pop("RUSTC", None)
                env.pop("CARGO_BUILD_RUSTC", None)
                configured = compiler if mode == "config" else poison_rustc
                config.write_text(
                    '[build]\ntarget = "wasm32-unknown-unknown"\n'
                    f'rustc = {json.dumps(configured.as_posix())}\n', encoding="utf-8")
                if mode != "config":
                    env[mode] = str(compiler)
                if mode == "RUSTC":
                    env["CARGO_BUILD_RUSTC"] = str(poison_rustc)
                command = [ARGS.bash, "--noprofile", "--norc", "-c", invoke, "test-launcher", str(poison)]
                before = subprocess.run([*command, str(old_probe)], env=env, cwd=self.root, capture_output=True, timeout=15)
                (self.root / f"compiler-{mode}-before.log").write_bytes(before.stdout + before.stderr)
                self.assertNotEqual(before.returncode, 0, "old bare-rustc probe unexpectedly succeeded")
                self.assertIn(b"Could not resolve rustc host target", before.stderr)
                after = subprocess.run([*command, str(launcher), "--full", "--plan"], env=env, cwd=RUN, capture_output=True, timeout=180)
                (self.root / f"compiler-{mode}-after.log").write_bytes(after.stdout + after.stderr)
                self.assertEqual(after.returncode, 0, after.stderr.decode(errors="replace"))
                self.assertTrue(any(s["name"] == "web-bundle" for s in json.loads(after.stdout)))
        # Missing compiler must fail even with a previously built host artifact.
        env["RUSTC"] = str(self.root / "missing-rustc")
        failed = subprocess.run([*command, str(launcher), "--plan"], env=env, cwd=RUN, capture_output=True, timeout=15)
        (self.root / "compiler-missing.log").write_bytes(failed.stdout + failed.stderr)
        self.assertNotEqual(failed.returncode, 0)
        self.assertEqual(failed.stdout, b"", "failed Cargo build must not execute a cached plan")

    def test_full_order_and_build_once_unicode_paths(self):
        self.run_cli("--full", "--no-serve", "--release")
        trace = (self.root / "trace.txt").read_text(encoding="utf-8")
        names = [line.split("\t")[0] for line in trace.splitlines()]
        self.assertEqual(names, ["wasm-check", "app-tests", "host-contracts", "web-tests", "browser-contracts", "web-bundle", "web-auth", "reader-theme-matrix", "small-viewport", "proxy-feed", "browser-feed"])
        self.assertEqual(names.count("web-bundle"), 1)
        self.assertIn("中文 space", trace)
        self.assertIn("结果 logs", trace)
        self.assertIn("--release", trace)
        self.assertEqual(self.stage("spa")["status"], "skipped")
        self.assertEqual(self.stage("web-browser-feed")["status"], "skipped")
        for log in ["automated-gates.log", "browser-contracts.log", "fixed-smokes.log"]:
            output = (self.log / log).read_text()
            self.assertIn("fixture stdout:", output)
            self.assertIn("fixture stderr:", output)

    def test_every_stage_failure_is_terminal_and_keeps_exit(self):
        stages = ["wasm-check", "app-tests", "host-contracts", "web-tests", "browser-contracts", "web-bundle", "web-auth", "reader-theme-matrix", "small-viewport", "proxy-feed", "browser-feed"]
        for stage in stages:
            with self.subTest(stage=stage):
                self.env["FAIL_STAGE"] = stage
                self.run_cli("--full", "--no-serve", code=23)
                report = self.report()
                self.assertEqual(report["outcome"], "failed")
                self.assertEqual(report["exit_code"], 23)
                failed = [s for s in report["steps"] if s["status"] == "failed"]
                self.assertEqual(len(failed), 1)
                index = report["steps"].index(failed[0])
                self.assertTrue(all(s["status"] in ["blocked", "skipped"] for s in report["steps"][index+1:]))
                self.assertFalse(any(s["status"] == "pending" for s in report["steps"]))
        self.env["FAIL_STAGE"] = "browser-feed"
        self.run_cli("--with-rssr-web", "--no-serve", code=23)
        self.assertEqual(self.stage("web-browser-feed")["status"], "failed")

    def test_skip_semantics_and_missing_bundle(self):
        self.run_cli("--skip-automated", "--skip-build", "--no-serve")
        self.assertTrue(all(s["status"] == "skipped" for s in self.report()["steps"]))
        self.run_cli("--skip-automated", "--skip-build", "--with-fixed-smokes", "--no-serve", code=1)
        self.assertEqual(self.stage("web-bundle")["status"], "failed")
        self.bundle()
        self.run_cli("--skip-automated", "--skip-build", "--with-fixed-smokes", "--no-serve")
        self.assertEqual(self.stage("web-bundle")["status"], "reused")
        self.assertEqual(self.stage("wasm-check")["status"], "skipped")
        self.assertNotIn("web-bundle", (self.root / "trace.txt").read_text(encoding="utf-8"))

    def test_local_fixture_selection_does_not_pass_external_probe(self):
        self.run_cli("--skip-external-feed", code=1)
        self.env["FAIL_STAGE"] = "proxy-feed"
        self.run_cli("--full", "--no-serve", "--skip-external-feed")
        self.assertEqual(self.stage("proxy-feed")["status"], "skipped")
        self.assertIsNone(self.stage("proxy-feed")["exit_code"])
        self.assertEqual(self.stage("fixed-browser-feed")["status"], "passed")
        self.assertEqual(self.report()["outcome"], "completed-with-skips")
        self.assertIn("partial (external feed skipped)", (self.log / "summary.md").read_text(encoding="utf-8"))
        self.assertNotIn("proxy-feed\t", (self.root / "trace.txt").read_text(encoding="utf-8"))
        self.env["FAIL_STAGE"] = "browser-feed"
        self.run_cli("--full", "--no-serve", "--skip-external-feed", code=23)
        self.assertEqual(self.stage("fixed-browser-feed")["status"], "failed")
        self.assertEqual(self.report()["outcome"], "failed")

    def test_missing_build_output_and_spawn_error(self):
        self.env["MISSING_BUNDLE"] = "1"
        self.run_cli("--skip-automated", "--with-fixed-smokes", "--no-serve", code=1)
        self.assertEqual(self.stage("web-bundle")["status"], "failed")
        self.env["RSSR_BASH"] = str(self.root / "missing-bash")
        self.run_cli("--skip-automated", "--with-browser-contracts", "--no-serve", code=1)
        self.assertEqual(self.stage("browser-contracts")["status"], "failed")

    def test_owned_descendants_after_success_and_failure(self):
        self.env.update(HOLD_STAGE="wasm-check", FIXTURE_PORT=str(free_port()), ORPHAN="1")
        for fail in ["", "wasm-check"]:
            self.env["FAIL_STAGE"] = fail
            for name in ["branch.pid", "leaf.pid", "tree-ready"]:
                (self.root / name).unlink(missing_ok=True)
            self.run_cli("--no-serve", code=23 if fail else 0)
            self.no_tree()

    def test_cancellation_and_forced_termination(self):
        events = ["ctrl-c", "ctrl-break", "git-bash-term", "terminate"] if WINDOWS else [signal.SIGINT, signal.SIGTERM]
        for event in events:
            with self.subTest(event=event):
                self.log = self.root / (f"event-{event} 结果 logs")
                self.env.update(HOLD_STAGE="wasm-check", FIXTURE_PORT=str(free_port()))
                for name in ["branch.pid", "leaf.pid", "tree-ready"]:
                    (self.root / name).unlink(missing_ok=True)
                proc = self.start("--full", "--no-serve")
                self.until(lambda: (self.root / "tree-ready").exists(), proc)
                if WINDOWS and event == "terminate":
                    proc.terminate()
                elif WINDOWS and event == "git-bash-term":
                    subprocess.run([ARGS.bash, "--noprofile", "--norc", "-c", '/usr/bin/kill -fW -TERM "$1"', "test-signal", str(proc.pid)], check=True, timeout=5)
                elif WINDOWS:
                    console_event(proc.pid, 0 if event == "ctrl-c" else 1)
                else:
                    proc.send_signal(event)
                code = proc.wait(timeout=8)
                (self.root / f"event-{event}.json").write_text(json.dumps({"event": str(event), "process_exit": code, "report_outcome": self.report()["outcome"], "stage_status": self.stage("wasm-check")["status"]}, indent=2), encoding="utf-8")
                if event in ["terminate", "git-bash-term"]:
                    self.assertNotEqual(code, 0)
                    self.assertIn(self.report()["outcome"], ["running", "failed", "interrupted"])
                    self.assertNotEqual(self.stage("wasm-check")["status"], "passed")
                else:
                    self.assertEqual(code, 130 if WINDOWS else 128 + event)
                    self.assertEqual(self.stage("wasm-check")["status"], "interrupted")
                    self.assertEqual(self.stage("app-tests")["status"], "blocked")
                self.no_tree()

    def test_spa_early_exit_and_port_conflict(self):
        self.bundle()
        self.env["FAIL_STAGE"] = "spa"
        self.run_cli("--skip-automated", "--skip-build", "--port", str(free_port()), code=23)
        self.assertEqual(self.stage("spa")["status"], "failed")
        with socket.socket() as listener:
            listener.bind(("127.0.0.1", 0))
            listener.listen()
            port = listener.getsockname()[1]
            with self.assertRaises(AssertionError):
                self.assert_port_free(port)
            self.run_cli("--skip-automated", "--skip-build", "--port", str(port), code=1)
            self.assertIn("unavailable", self.stage("spa")["detail"])
            self.assertEqual(listener.getsockname()[1], port)

    def test_spa_readiness_and_cancel(self):
        self.bundle()
        port = free_port()
        self.env.update(HOLD_STAGE="spa", FIXTURE_PORT=str(port))
        proc = self.start("--skip-automated", "--skip-build", "--port", str(port))
        self.until(lambda: self.stage("spa")["status"] == "serving", proc)
        self.assertEqual(self.report()["outcome"], "running")
        if WINDOWS:
            console_event(proc.pid, 1)
        else:
            proc.send_signal(signal.SIGTERM)
        self.assertEqual(proc.wait(timeout=8), 130 if WINDOWS else 143)
        self.assertEqual(self.stage("spa")["status"], "interrupted")
        self.no_tree()

    def real_scripts(self):
        self.env["RSSR_BASH"] = ARGS.bash
        for name in ["run_web_spa_regression_server.sh", "run_rssr_web_auth_smoke.sh"]:
            shutil.copy2(ROOT / "scripts" / name, self.root / "scripts" / name)
        self.bundle()
        (self.root / "target/dx/rssr-app/debug/web/public/index.html").write_text("real SPA fixture", encoding="utf-8")

    def test_real_auth_service_early_exit_and_conflict(self):
        self.real_scripts()
        self.env["FAIL_STAGE"] = "cargo-run"
        self.run_cli("--skip-automated", "--skip-build", "--no-serve", "--with-rssr-web", "--web-port", str(free_port()), code=1)
        self.assertEqual(self.stage("web-auth")["status"], "failed")
        self.assertIn("exited before readiness", (self.log / "rssr-web-auth-smoke.log").read_text(encoding="utf-8"))
        with socket.socket() as listener:
            listener.bind(("127.0.0.1", 0))
            listener.listen()
            self.run_cli("--skip-automated", "--skip-build", "--no-serve", "--with-rssr-web", "--web-port", str(listener.getsockname()[1]), code=1)
            self.assertIn("unavailable", self.stage("web-auth")["detail"])

    def test_real_spa_with_bash_python_and_cancel(self):
        self.real_scripts()
        port = free_port()
        proc = self.start("--skip-automated", "--skip-build", "--port", str(port))
        self.until(lambda: self.stage("spa")["status"] == "serving", proc)
        stage_pid = self.stage("spa")["pid"]
        if WINDOWS:
            console_event(proc.pid, 0)
        else:
            proc.send_signal(signal.SIGINT)
        self.assertEqual(proc.wait(timeout=8), 130)
        self.assertEqual(self.stage("spa")["status"], "interrupted")
        self.until(lambda: not alive(stage_pid))
        self.assert_port_free(port)


if __name__ == "__main__":
    print(f"Acceptance artifacts: {RUN}", flush=True)
    unittest.main(argv=[sys.argv[0], *remaining], verbosity=2)
