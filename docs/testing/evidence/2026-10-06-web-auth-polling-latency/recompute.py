"""Audit saved summaries; optionally parse an already extracted raw archive."""
import argparse
import json
from pathlib import Path
from summarize import describe, runtime_summary, segments_summary
from parse_traces import parse
HERE = Path(__file__).resolve().parent
def read(name):
    return json.loads((HERE / name).read_text())
def check(name, actual):
    assert read(name) == actual, name
    print("verified", name)
if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--traces", type=Path, help="extracted archive evidence/ directory")
    args = parser.parse_args()
    runtime = read("final-runtime-raw.json")
    assert len(runtime) == 90 and all(not r["traced"] for r in runtime)
    for label in "ABC":
        assert len([r for r in runtime if r["label"] == label]) == 30
    check("final-runtime-summary.json", runtime_summary(runtime))
    raw = read("attribution-raw.json")
    at = segments_summary(read("attribution-segments.json"))
    for label in "AB":
        traced = {r["pair"]: r for r in raw if r["label"] == label and r["traced"]}
        control = {r["pair"]: r for r in raw if r["label"] == label and not r["traced"]}
        assert len(traced) == len(control) == 20 and traced.keys() == control.keys()
        at[label]["traced_minus_control_stage_ms"] = describe(
            [traced[p]["stage_ms"] - control[p]["stage_ms"] for p in sorted(traced)])
        at[label]["control_stage_ms"] = describe([r["stage_ms"] for r in control.values()])
        at[label]["control_cpu_ms"] = describe(
            [(r["cpu_user_s"] + r["cpu_system_s"]) * 1000 for r in control.values()])
    check("attribution-summary.json", at)
    check("post-trace-summary.json", segments_summary(read("post-trace-segments.json")))
    for filename, prefix in [("attribution-segments.json", "attribution-traced"),
                              ("post-trace-segments.json", "post-trace")]:
        rows = read(filename)
        assert len(rows) == 40
        for row in rows:
            assert row["authentication_curl_count"] == 7 and row["grep_count"] == 10
            if args.traces:
                trace_dir = args.traces / f'{prefix}-{row["pair"]}-{row["label"]}-trace'
                rebuilt = dict(label=row["label"], pair=row["pair"],
                               stage_ms=row["stage_ms"], **parse(trace_dir))
                assert row == rebuilt, trace_dir
        if args.traces:
            print("verified 40 raw traces", filename)
    for row in runtime + raw + read("post-trace-raw.json"):
        assert row["exit_code"] == 0 and row["port_closed"]
        assert not row["remaining_service_pids"]
    print("all 210 planned measurements: successful, no recorded service/port residue")
