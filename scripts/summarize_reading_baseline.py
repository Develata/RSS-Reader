"""Summarize explicitly named native baseline batches, retaining per-run values."""
import argparse
import json
import math
from pathlib import Path
import re
import statistics


def distribution(values):
    return {"median": statistics.median(values), "min": min(values), "max": max(values), "values": values}


def summarize(root, prefix):
    result = {}
    runs = sorted(root.glob(f"{prefix}-[0-9][0-9]-[ab]"))
    for variant in ("a", "b"):
        paths = [p for p in runs if p.name.endswith(f"-{variant}")]
        if len(paths) != 3:
            raise ValueError(f"Expected three {prefix}/{variant} runs, found {len(paths)}")
        records = [json.loads((p / "measurements.json").read_text(encoding="utf-8")) for p in paths]
        if any(r["status"] != "passed" or r["errors"] for r in records):
            raise ValueError(f"Failed run in {prefix}/{variant}")
        identities = {(r["metadata"]["source"], r["metadata"]["artifact-sha"]) for r in records}
        if len(identities) != 1:
            raise ValueError("Mixed artifact identities")
        source, artifact = identities.pop()
        views = {tuple(r["environment"]["viewport"]) for r in records}
        if len(views) != 1:
            raise ValueError("Viewport changed between runs")
        item = {"source": source, "artifactSha256": artifact, "runs": [p.name for p in paths],
                "viewport": list(views.pop()), "timingMs": {}, "memoryMiB": {},
                "startupObservedUpperBoundMs": distribution([r["startupObservedMs"] for r in records]),
                "directoryNodes": [r["directory"] for r in records],
                "memoryRetries": [len(r.get("memoryRetries", [])) for r in records]}
        names = {"search-one", "search-clear", "directory-source-open", "reader-open", "reader-return"}
        if any({t["name"] for t in r["timings"]} != names for r in records):
            raise ValueError("Missing or unexpected operation")
        for name in sorted(names):
            batches = [[t for t in r["timings"] if t["name"] == name] for r in records]
            counts = [int(r["metadata"].get("reader-samples", r["metadata"].get("samples", 5)))
                      if name.startswith("reader-") else int(r["metadata"].get("samples", 5)) for r in records]
            if len(set(counts)) != 1 or any(len(b) != n or n < 1 for b, n in zip(batches, counts)):
                raise ValueError(f"Invalid sample counts for {name}")
            item["timingMs"][name] = {key: distribution([statistics.median(t[key] for t in b) for b in batches])
                                      for key in ("domMs", "proxyMs")}
            item["timingMs"][name]["samplesPerRun"] = counts[0]
        for state in ("list", "directory", "reader", "returned"):
            batches = [[m for m in r["memory"] if m["state"] == state] for r in records]
            if any(len(b) != 3 for b in batches):
                raise ValueError(f"Expected three memory samples for {state}")
            android = records[0]["metadata"]["platform"] == "android"
            key = "totalPssKiB" if android else "privateWorkingSetBytes"
            divisor = 1024 if android else 1024 * 1024
            item["memoryMiB"][state] = distribution([statistics.median(m[key] / divisor for m in b) for b in batches])
        gaps = [[t[i] - t[i-1] for s in r["scroll"] for t in [s["frames"]] for i in range(1, len(t))] for r in records]
        item["scrollRafGapP95Ms"] = distribution([sorted(g)[math.ceil(.95 * len(g)) - 1] for g in gaps])
        if records[0]["metadata"]["platform"] == "android":
            item["activityFirstDrawMs"] = distribution([
                int(re.search(r"TotalTime:\s*(\d+)", (p / "launch.txt").read_text(encoding="utf-8-sig"))[1]) for p in paths])
            item["temperatureC"] = [float(re.search(r"temperature:\s*(\d+)",
                (p / "battery-before.txt").read_text(encoding="utf-8-sig"))[1]) / 10 for p in paths]
        result[variant] = item
    if result["a"]["viewport"] != result["b"]["viewport"]:
        raise ValueError("A/B viewports differ")
    if any(result["a"]["timingMs"][name]["samplesPerRun"] != result["b"]["timingMs"][name]["samplesPerRun"]
           for name in result["a"]["timingMs"]):
        raise ValueError("A/B operation counts differ")
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", type=Path)
    parser.add_argument("--windows-prefix", default="windows-final-v2")
    parser.add_argument("--android-prefix", default="android-final-v2")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    report = {"method": "Median of three run medians; raw values retained. Descriptive comparison, not significance testing.",
              "windows": summarize(args.root, args.windows_prefix),
              "android": summarize(args.root, args.android_prefix)}
    args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(args.output)


if __name__ == "__main__":
    main()
