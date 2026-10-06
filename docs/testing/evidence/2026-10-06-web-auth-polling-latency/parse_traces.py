"""Parse syscall diagnostics; traced durations never replace untraced A/B."""
import re, json
from pathlib import Path

def parse(directory):
    processes={}
    birth={}
    reaped={}
    for file in directory.glob("pid.*"):
        pid=int(file.name.split(".")[-1]); rows=[]
        for line in file.read_text().splitlines():
            m=re.match(r"^(\d+\.\d+) (.*)$",line)
            if not m:continue
            text=m[2]; d=re.search(r"<(\d+\.\d+)>$",text)
            row={"at":float(m[1]),"duration":float(d[1]) if d else 0,"text":text}
            row["end"]=row["at"]+row["duration"];rows.append(row)
        processes[pid]=rows
    def returned(text):
        m=re.search(r" = (-?\d+)(?: |$)",text)
        return int(m[1]) if m else None
    for pid,rows in processes.items():
        for row in rows:
            text=row["text"];ret=returned(text)
            if text.startswith(("clone(","clone3(","vfork(","fork(")) and ret and ret>0 and "CLONE_THREAD" not in text:
                birth[ret]=dict(row,parent=pid)
            if text.startswith("wait4(") and ret and ret>0:
                reaped.setdefault(ret,[]).append(dict(row,parent=pid))
    def execs(pid):
        return [r for r in processes[pid] if r["text"].startswith("execve(") and returned(r["text"])==0]
    def matches(fragment):
        return [pid for pid in processes if any(fragment in r["text"] for r in execs(pid))]
    root=next(pid for pid in processes if any('/bin-' in r["text"] for r in execs(pid)))
    service=next(pid for pid in processes if any('/rssr-web"' in r["text"] for r in execs(pid)))
    ready=sorted(matches("/healthz"),key=lambda pid:birth[pid]["at"])
    curl=[pid for pid in processes if any(re.match(r'execve\(".*?/curl"',r["text"]) for r in execs(pid))]
    auth=sorted(set(curl)-set(ready),key=lambda pid:birth[pid]["at"])
    grep=[pid for pid in processes if any(re.match(r'execve\(".*?/grep"',r["text"]) for r in execs(pid))]
    adapter=matches("scripts/run_rssr_web_auth_assertions.sh") or matches("scripts/run_rssr_web_auth_smoke.sh")
    assert len(adapter)==1
    adapter=adapter[0]
    def end(pid):
        return next(r["at"] for r in reversed(processes[pid]) if r["text"].startswith("+++ "))
    def observed(pid):
        return min(reaped[pid],key=lambda r:r["end"])
    def describe_child(pid):
        born=birth[pid];obs=observed(pid);exited=end(pid)
        polls=[r for r in processes[born["parent"]] if r["text"].startswith(f"wait4({pid},") and "WNOHANG" in r["text"] and born["at"]<=r["at"]<=obs["at"]]
        return {"pid":pid,"parent":born["parent"],"spawn_at":born["at"],"spawn_syscall_ms":born["duration"]*1000,
                "exit_at":exited,"observed_at":obs["end"],"exit_observation_ms":(obs["end"]-exited)*1000,
                "spawn_to_observed_ms":(obs["end"]-born["at"])*1000,"nonblocking_wait_calls":len(polls),
                "exit_record":next(r["text"] for r in reversed(processes[pid]) if r["text"].startswith("+++ "))}
    ready_rows=[describe_child(pid) for pid in ready]
    root_rows=processes[root]
    service_parent=birth[service]["parent"]
    signal=next(r for r in processes[service_parent] if re.match(rf"kill\(-?{service}, SIGTERM\)",r["text"]))
    service_obs=observed(service)
    checks=[r for r in processes[service_parent] if r["at"]>=signal["at"] and r["text"].startswith(f"kill(-{service}, 0)")]
    stop_end=max([service_obs["end"]]+[r["end"] for r in checks])
    retries=[]
    for previous,nxt in zip(ready_rows,ready_rows[1:]):
        lo=previous["observed_at"];hi=nxt["spawn_at"]
        sleeps=[r for r in root_rows if lo<=r["at"]<=hi and r["text"].startswith("clock_nanosleep")]
        retries.append({"from":lo,"to":hi,"gap_ms":(hi-lo)*1000,"root_sleep_calls":len(sleeps)})
    after_service_exec=next(r for r in execs(service) if '/rssr-web"' in r["text"])
    start=birth[adapter]["at"] if service_parent!=root else birth[service]["at"]
    root_polls=[r for r in root_rows if start<=r["at"]<=max(stop_end,observed(adapter)["end"]) and r["text"].startswith("wait4(") and "WNOHANG" in r["text"]]
    # The last grep is the final logout assertion in both unchanged contracts.
    last_grep=max(grep,key=end)
    return {"root_pid":root,"service":describe_child(service),
            "service_spawn_syscall_ms":birth[service]["duration"]*1000,
            "cargo_until_product_exec_ms":(after_service_exec["at"]-birth[service]["at"])*1000,
            "readiness_count":len(ready),"readiness":ready_rows,"retry":retries,
            "readiness_exit_observation_ms":sum(r["exit_observation_ms"] for r in ready_rows),
            "readiness_spawn_to_observed_ms":(ready_rows[-1]["observed_at"]-ready_rows[0]["spawn_at"])*1000,
            "authentication_curl_count":len(auth),"grep_count":len(grep),
            "assertion_core_ms":(end(last_grep)-birth[auth[0]]["at"])*1000,
            "owned_adapter_or_outer_shell":describe_child(adapter),
            "service_stop_ms":(stop_end-signal["at"])*1000,
            "service_stop_signal_at":signal["at"],"service_stop_end_at":stop_end,
            "service_group_probe_count":len(checks),
            "service_stop_wait_calls":sum(signal["at"]<=r["at"]<=stop_end and r["text"].startswith((f"wait4({service},", "wait4(-1,")) for r in processes[service_parent]),
            "root_nonblocking_wait_calls_in_stage":len(root_polls),
            "trace_files":len(processes)}
