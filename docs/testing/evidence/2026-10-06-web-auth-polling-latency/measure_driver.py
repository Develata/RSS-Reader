"""One-off WSL experiment. No cache cleanup; traced samples are diagnostic only."""
import os, json, socket, subprocess, time
from pathlib import Path
ROOT=Path("/home/deve/gitclone/RSS-Reader")
BASE=ROOT/"target/tool-migration-20261006-polling"
OUT=BASE/"evidence"
PRODUCT_TARGET=ROOT/"target/tool-migration-20261006/product-build"
ENV=dict(os.environ,PATH=str(BASE/"tools")+os.pathsep+os.environ["PATH"],
         CARGO_TARGET_DIR=str(PRODUCT_TARGET),CARGO_NET_OFFLINE="true",
         NO_PROXY="127.0.0.1,localhost",no_proxy="127.0.0.1,localhost")
def free_ports():
    while True:
        with socket.socket() as a:
            a.bind(("127.0.0.1",0)); p=a.getsockname()[1]
            if p==65535: continue
            with socket.socket() as b:
                try: b.bind(("127.0.0.1",p+1)); return p
                except OSError: pass
def measure(label,series,pair,traced=False):
    root=BASE/(label+" 中文 space")
    tag=f"{series}-{pair}-{label}"
    logs=root/tag
    port=free_ports()
    cmd=[str(BASE/("bin-"+label)),"--repo-root",str(root),"--skip-automated",
         "--skip-build","--no-serve","--with-rssr-web","--web-port",str(port),
         "--log-dir",str(logs)]
    if traced:
        traces=OUT/(tag+"-trace");traces.mkdir()
        cmd=["strace","-ff","--seccomp-bpf","-ttt","-T","-s","512",
             "-e","trace=process,clock_nanosleep,nanosleep,kill",
             "-o",str(traces/"pid"),*cmd]
    started=time.perf_counter_ns()
    with (OUT/(tag+".log")).open("wb") as output:
        proc=subprocess.Popen(cmd,cwd=root,env=ENV,stdout=output,stderr=subprocess.STDOUT)
        os.waitid(os.P_PID,proc.pid,os.WEXITED|os.WNOWAIT)
        # The unreaped zombie retains scheduler counters. For untraced runs this
        # is the supervisor MAIN THREAD, not its console thread or descendants.
        sched=[int(x) for x in Path(f"/proc/{proc.pid}/schedstat").read_text().split()]
        stat=Path(f"/proc/{proc.pid}/stat").read_text().split(") ",1)[1].split()
        self_ticks=int(stat[11])+int(stat[12])
        _,status,usage=os.wait4(proc.pid,0)
        proc.returncode=os.waitstatus_to_exitcode(status)
    rec={"label":label,"series":series,"pair":pair,"traced":traced,
         "wall_ns":time.perf_counter_ns()-started,"command":cmd,"exit_code":proc.returncode,
         "main_thread_run_ns":sched[0] if not traced else None,
         "main_thread_runqueue_ns":sched[1] if not traced else None,
         "main_thread_timeslices":sched[2] if not traced else None,
         "supervisor_process_cpu_ticks":self_ticks if not traced else None,
         "cpu_user_s":usage.ru_utime,"cpu_system_s":usage.ru_stime,
         "maxrss_kib":usage.ru_maxrss,"voluntary_context_switches":usage.ru_nvcsw,
         "involuntary_context_switches":usage.ru_nivcsw}
    report=logs/"summary.json"
    if report.exists():
        stage=next(x for x in json.loads(report.read_text())["steps"] if x["name"]=="web-auth")
        rec.update(stage_ms=stage["duration_ms"],stage_status=stage["status"])
    with socket.socket() as probe:
        probe.settimeout(.2); rec["port_closed"]=probe.connect_ex(("127.0.0.1",port))!=0
    rec["remaining_service_pids"]=[]
    for p in Path("/proc").iterdir():
        if not p.name.isdigit():continue
        try:
            if (p/"exe").resolve()==PRODUCT_TARGET/"debug/rssr-web":
                rec["remaining_service_pids"].append(int(p.name))
        except OSError:pass
    (OUT/(tag+".json")).write_text(json.dumps(rec,indent=2))
    assert proc.returncode==0,rec
    assert rec["port_closed"] and not rec["remaining_service_pids"],rec
    return rec
