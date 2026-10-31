#!/usr/bin/env python3
"""Compile and run the production Settings rules without a UI framework or host adapter."""
import argparse
import json
import os
from pathlib import Path
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target-dir", type=Path, default=Path("target/settings-rule-check"))
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    target = args.target_dir.resolve()
    target.mkdir(parents=True, exist_ok=True)
    # Build the real settings types, patch operations and CSS validator; no app artifacts,
    # stand-in model/validator or additional Cargo package is used by this probe.
    build = subprocess.run(
        ["cargo", "build", "-p", "rssr-api", "--locked", "--target-dir", str(target),
         "--message-format=json"], cwd=root, text=True, encoding="utf-8",
        stdout=subprocess.PIPE, check=True,
    )
    libraries = {}
    compiled = set()
    for line in build.stdout.splitlines():
        message = json.loads(line)
        if message.get("reason") != "compiler-artifact":
            continue
        name = message["target"]["name"]
        compiled.add(name)
        for filename in message["filenames"]:
            if filename.endswith(".rlib"):
                libraries[name] = Path(filename)
    forbidden = {name for name in compiled if name.startswith(("dioxus", "web_sys", "wry", "tao"))}
    if forbidden:
        raise RuntimeError(f"UI dependencies entered the rule build: {sorted(forbidden)}")
    required = ("rssr_api", "rssr_application", "rssr_domain")
    executable = target / ("settings-rules.exe" if os.name == "nt" else "settings-rules")
    command = ["rustc", "--edition=2024", "--test", "--deny=warnings",
               "--crate-name=settings_rules", "crates/rssr-app/tests/settings_rules.rs",
               "-o", str(executable)]
    for name in required:
        command.extend(["--extern", f"{name}={libraries[name]}"])
    for directory in sorted({str(libraries[name].parent) for name in required}):
        command.extend(["-L", f"dependency={directory}"])
    subprocess.run(command, cwd=root, check=True)
    tests = subprocess.run([str(executable)], cwd=root, text=True, encoding="utf-8",
                           stdout=subprocess.PIPE, check=True)
    print(tests.stdout, end="")
    receipt = {"externCrates": list(required), "compiledDependencies": sorted(compiled),
               "rustc": command, "result": tests.stdout, "frameworkFree": True}
    (target / "settings-rule-boundary.json").write_text(
        json.dumps(receipt, ensure_ascii=False, indent=2), encoding="utf-8"
    )


if __name__ == "__main__":
    main()
