"""WSL-only paired measurements. Run after freezing the candidate commit.
No cache deletion. Warmups/builds are recorded separately from runtime samples.
The auth pair uses real rssr-web, a common cwd/Cargo shim, and a stubbed later
browser stage. It is not a full browser/UI regression or isolated readiness metric.
"""
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import socket
import statistics
import subprocess
import time

ROOT = Path("/home/deve/gitclone/RSS-Reader")
BASE = ROOT / "target/tool-migration-20261005"
OUT = BASE / "evidence"
A = "f69087a61a4627cbd3ba88fa082d7722df811dc5"
B = subprocess.check_output(["git", "-C", str(ROOT), "rev-parse", "HEAD"], text=True).strip()
assert B != A, "commit the candidate before measuring it"
assert not subprocess.check_output(["git", "-C", str(ROOT), "diff", "--", "scripts"])
assert not subprocess.check_output(["git", "-C", str(ROOT), "diff", "--cached", "--", "scripts"])
SOURCES = {"A": BASE / "baseline-source", "B": ROOT}
BINS = {label: BASE / (kind + "-build/debug/release-ui")
        for label, kind in [("A", "baseline"), ("B", "candidate")]}
ENV = dict(os.environ, CARGO_NET_OFFLINE="true", NO_PROXY="127.0.0.1,localhost", no_proxy="127.0.0.1,localhost")
roots = {}
for label in ["A", "B"]:
    root = BASE / "benchmarks" / (label + " 中文 space")
    root.mkdir(parents=True, exist_ok=True)
    roots[label] = root
    if not (root / "scripts").exists():
        shutil.copytree(SOURCES[label] / "scripts/release-ui", root / "scripts/release-ui",
                        ignore=shutil.ignore_patterns("target", "__pycache__"))
        for name in ["run_release_ui_regression.sh", "run_rssr_web_auth_smoke.sh",
                     "run_web_spa_regression_server.sh"]:
            shutil.copy2(SOURCES[label] / "scripts" / name, root / "scripts" / name)
        if label == "B":
            shutil.copy2(ROOT / "scripts/run_rssr_web_auth_assertions.sh", root / "scripts")
        (root / "scripts/run_rssr_web_browser_feed_smoke.sh").write_text(
            "#!/bin/bash\n# Excluded from auth measurement; no browser claim.\nexit 0\n")
        public = root / "target/dx/rssr-app/debug/web/public"
        public.mkdir(parents=True)
        shutil.copy2(ROOT / "target/dx/rssr-app/debug/web/public/index.html", public / "index.html")

shim = BASE / "benchmark-tools"
shim.mkdir(exist_ok=True)
(shim / "cargo").write_text(
    '#!/bin/bash\ncd "/home/deve/gitclone/RSS-Reader"\nexec /usr/bin/cargo "$@"\n')
(shim / "cargo").chmod(0o755)
AUTH_ENV = dict(ENV, PATH=str(shim) + os.pathsep + ENV["PATH"],
                CARGO_TARGET_DIR=str(BASE / "product-build"))
product = BASE / "product-build/debug/rssr-web"

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

meta = {"A": A, "B": B, "platform": platform.platform(), "root": str(ROOT),
        "rustc": subprocess.check_output(["rustc", "-Vv"], text=True),
        "cargo": subprocess.check_output(["cargo", "-V"], text=True),
        "binary_sha256": {k: digest(p) for k, p in BINS.items()},
        "binary_bytes": {k: p.stat().st_size for k, p in BINS.items()},
        "product_sha256": digest(product),
        "bundle_index_sha256": digest(ROOT / "target/dx/rssr-app/debug/web/public/index.html"),
        "auth_scope": "real web-auth + stub later browser; common Cargo cwd shim; reused bundle",
        "readiness": "included in auth stage; not independently measured",
        "no_p99_or_memory_claim": True, "pairs": 20, "warmups": [], "samples": []}

def save():
    (OUT / "paired-raw.json").write_text(json.dumps(meta, indent=2), encoding="utf-8")

def port_pair():
    while True:
        with socket.socket() as a:
            a.bind(("127.0.0.1", 0))
            port = a.getsockname()[1]
            if port == 65535:
                continue
            with socket.socket() as b:
                try:
                    b.bind(("127.0.0.1", port + 1))
                    return port
                except OSError:
                    pass

def leftovers():
    found = []
    for entry in Path("/proc").iterdir():
        if not entry.name.isdigit():
            continue
        try:
            if (entry / "exe").resolve() == product:
                found.append(int(entry.name))
        except OSError:
            pass
    return found

def measure(kind, label, pair, port=None):
    root = roots[label]
    suffix = f"{kind}-{pair}-{label}"
    if kind == "direct-plan":
        cmd = [str(BINS[label]), "--full", "--plan"]
        env = ENV
    elif kind == "noop-build":
        build = BASE / (("baseline" if label == "A" else "candidate") + "-build")
        cmd = ["cargo", "build", "--locked", "--offline", "--manifest-path",
               str(SOURCES[label] / "scripts/release-ui/Cargo.toml"), "--target-dir", str(build)]
        env = ENV
    elif kind == "wrapper-plan":
        cmd = ["/bin/bash", str(root / "scripts/run_release_ui_regression.sh"), "--full", "--plan"]
        env = ENV
    else:
        logs = root / suffix
        cmd = [str(BINS[label]), "--repo-root", str(root), "--skip-automated",
               "--skip-build", "--no-serve", "--with-rssr-web",
               "--web-port", str(port), "--log-dir", str(logs)]
        env = AUTH_ENV
    start = time.perf_counter_ns()
    with (OUT / (suffix + ".log")).open("wb") as out:
        proc = subprocess.run(cmd, cwd=root, env=env, stdout=out, stderr=subprocess.STDOUT,
                              timeout=400 if kind == "web-auth" else 180)
    rec = {"kind": kind, "label": label, "pair": pair, "ns": time.perf_counter_ns() - start,
           "exit_code": proc.returncode, "command": cmd}
    if kind == "web-auth":
        report = json.loads((logs / "summary.json").read_text())
        stage = next(s for s in report["steps"] if s["name"] == "web-auth")
        rec["web_auth_stage_ms"] = stage["duration_ms"]
        rec["stage_status"] = stage["status"]
        rec["remaining_service_pids"] = leftovers()
        with socket.socket() as probe:
            probe.settimeout(.2)
            rec["port_closed"] = probe.connect_ex(("127.0.0.1", port)) != 0
        if rec["remaining_service_pids"] or not rec["port_closed"]:
            meta["blocked"] = rec
            save()
            raise SystemExit("process or listener residue: stop")
    (meta["warmups"] if pair < 0 else meta["samples"]).append(rec)
    save()
    if proc.returncode:
        raise SystemExit(f"failed sample: {rec}")
    return rec

for kind in ["direct-plan", "noop-build", "wrapper-plan", "web-auth"]:
    port = port_pair() if kind == "web-auth" else None
    for label in ["A", "B"]:
        print("warmup", measure(kind, label, -1, port)["ns"] / 1e6, label, kind, flush=True)
    for pair in range(20):
        port = port_pair() if kind == "web-auth" else None
        order = ["A", "B"] if pair % 2 == 0 else ["B", "A"]
        for label in order:
            rec = measure(kind, label, pair, port)
        if kind == "web-auth":
            print("auth pair", pair + 1, flush=True)

def describe(values):
    q = statistics.quantiles(values, n=4, method="inclusive")
    median = statistics.median(values)
    return {"n": len(values), "median_ms": median, "q1_ms": q[0], "q3_ms": q[2],
            "iqr_ms": q[2] - q[0], "mad_ms": statistics.median(abs(x - median) for x in values),
            "min_ms": min(values), "max_ms": max(values)}

stats = {}
for kind in ["direct-plan", "noop-build", "wrapper-plan", "web-auth"]:
    rows = [r for r in meta["samples"] if r["kind"] == kind]
    values = {label: {r["pair"]: r["ns"] / 1e6 for r in rows if r["label"] == label}
              for label in ["A", "B"]}
    stats[kind] = {label: describe(list(v.values())) for label, v in values.items()}
    stats[kind]["paired_B_minus_A"] = describe([values["B"][i] - values["A"][i] for i in range(20)])
    stats[kind]["failures"] = sum(r["exit_code"] != 0 for r in rows)
(OUT / "paired-summary.json").write_text(json.dumps(stats, indent=2))
print(json.dumps(stats, indent=2), flush=True)
