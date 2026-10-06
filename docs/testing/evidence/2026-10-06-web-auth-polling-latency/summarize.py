"""Statistics for fixed-version one-off measurements; no dropped samples."""
import json, statistics
from pathlib import Path
BASE=Path("/home/deve/gitclone/RSS-Reader/target/tool-migration-20261006-polling")
OUT=BASE/"evidence"
def describe(values):
    med=statistics.median(values)
    q=statistics.quantiles(values,n=4,method="inclusive")
    return {"n":len(values),"median":med,"q1":q[0],"q3":q[2],"iqr":q[2]-q[0],
            "mad":statistics.median(abs(v-med) for v in values),"min":min(values),"max":max(values)}
def runtime_summary(rows):
    getters={"stage_ms":lambda r:r["stage_ms"],"wall_ms":lambda r:r["wall_ns"]/1e6,
             "main_thread_cpu_ms":lambda r:r["main_thread_run_ns"]/1e6,
             "waited_cpu_ms":lambda r:(r["cpu_user_s"]+r["cpu_system_s"])*1000,
             "maxrss_kib":lambda r:r["maxrss_kib"],
             "main_thread_timeslices":lambda r:r["main_thread_timeslices"]}
    labels=sorted(set(r["label"] for r in rows));result={}
    for metric,get in getters.items():
        values={label:{r["pair"]:get(r) for r in rows if r["label"]==label} for label in labels}
        stats={k:describe(list(v.values())) for k,v in values.items()}
        for left,right in [("C","B"),("C","A"),("B","A")]:
            if left not in values or right not in values:continue
            assert values[left].keys()==values[right].keys()
            differences=[values[left][p]-values[right][p] for p in sorted(values[left])]
            stats[left+"_minus_"+right]=dict(describe(differences),
                negative=sum(x<0 for x in differences),zero=sum(x==0 for x in differences),
                positive=sum(x>0 for x in differences))
        result[metric]=stats
    result["failures"]=sum(r["exit_code"]!=0 for r in rows)
    result["residue_failures"]=sum(not r["port_closed"] or bool(r["remaining_service_pids"]) for r in rows)
    return result
def segments_summary(rows):
    result={}
    for label in sorted(set(r["label"] for r in rows)):
        chosen=[r for r in rows if r["label"]==label]
        stats={k:describe([r[k] for r in chosen]) for k in [
            "service_spawn_syscall_ms","cargo_until_product_exec_ms","readiness_count",
            "readiness_exit_observation_ms","readiness_spawn_to_observed_ms","assertion_core_ms",
            "service_stop_ms","service_group_probe_count","root_nonblocking_wait_calls_in_stage"]}
        stats["owned_exit_delay_ms"]=describe([r["owned_adapter_or_outer_shell"]["exit_observation_ms"] for r in chosen])
        stats["retry_gap_ms"]=describe([d["gap_ms"] for r in chosen for d in r["retry"]])
        stats["readiness_poll_calls"]=describe([sum(d["nonblocking_wait_calls"] for d in r["readiness"]) for r in chosen])
        result[label]=stats
    return result
if __name__=="__main__":
    raw=json.loads((OUT/"final-runtime-raw.json").read_text())
    assert len(raw)==90
    result=runtime_summary(raw)
    (OUT/"final-runtime-summary.json").write_text(json.dumps(result,indent=2))
    old=json.loads((OUT/"attribution-segments.json").read_text())
    (OUT/"attribution-segment-summary.json").write_text(json.dumps(segments_summary(old),indent=2))
    print(json.dumps(result,indent=2))
