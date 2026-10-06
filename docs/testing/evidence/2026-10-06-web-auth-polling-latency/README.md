# web-auth bounded polling: fixed-version evidence (2026-10-06)

One-off measurement record for Draft PR #23. The Python files here analyze or
reproduce the experiment; they are not shipping tool dependencies. Implementation
is limited to two finite early-sleep sequences in release-ui.

## Fixed versions and inputs

| Label | Commit | Meaning |
| --- | --- | --- |
| A | `f69087a61a4627cbd3ba88fa082d7722df811dc5` | Original Bash-owned auth stage |
| B | `1b710aed0276f5b07558ea94ae2698b385353a2b` | Before optimization; runtime is e6915a6 |
| C | `4cc564201503b0e1e03413f45cb257f548e55037` | Finite early readiness/Unix-stop sleeps |

[environment.json](environment.json) records binary sizes/hashes, product binary
hash, index hash, toolchain and kernel. A/B binaries were retained from the
previous experiment; hashes stayed fixed throughout. C is the fresh clean-build
binary. Rebuilt B is measured only for compile cost.

All timed runs use Debian WSL's Linux filesystem at
`/home/deve/gitclone/RSS-Reader/target/tool-migration-20261006-polling` on the
E-backed WSL disk. Host i7-12700H; WSL exposes 20 logical CPUs.
Rust/Cargo 1.97.0, strace 6.13. No affinity, global scheduler changes, cache
purges or C-drive build target. Native Windows is correctness evidence only;
its timings are not included in these comparisons.

Each `A/B/C 中文 space` directory contains its commit's `git archive ... scripts`
snapshot. All three contain the **same copied index.html from the existing
debug bundle**, not a rebuilt/full browser asset bundle. The subsequent browser
stage is stubbed with `exit 0` so the recorded internal web-auth duration isolates
service startup, readiness, auth assertions and cleanup. This does not measure
complete browser/aggregate regression. A common `tools/cargo` shim changes cwd
to the product checkout then execs `/usr/bin/cargo "$@"`; every stage still runs
the same `cargo run --locked -p rssr-web` with the same already-built product
target. Product and index hashes were checked after the final series.
No release-mode flag, network feed or product input was changed.

## Design and measurement boundaries

1. Exploratory `diagnose-0-A/B`: retained separately. A had K=3 readiness requests
   and about 1.13 s cargo-to-product exec. Not included in the planned series.
2. Attribution: 20 groups, each A-control, A-traced, B-traced, B-control;
   reverse the entire order on odd groups. 80 successful executions. A/B each
   have 20 traced and 20 adjacent untraced samples. K was measured as 2 in all
   40 traces, with 7 auth curl and 10 grep processes. Untraced K is unknown;
   earlier experiments must not be retroactively assigned K=2.
3. Final runtime: one preplanned warmup each A/B/C, saved separately, then all
   six order permutations repeated five times (90 executions):
   ABC, CBA, BCA, ACB, CAB, BAC. Each pair is balanced 15/15 and each label
   occurs ten times in each position. No measured sample was discarded.
4. Post-change attribution: 20 B/C traced pairs, BC/CB alternating. K=2,
   7 auth curl and 10 grep in each of 40 traces. This checks the location of
   the savings, not uninstrumented total speed.

`stage_ms` is the unchanged public summary's internal web-auth duration.
`wall_ns` includes startup, the fixed browser stub and report completion.
Final wall uses blocking waitid(WNOWAIT), reads zombie counters, then wait4;
it includes those reads. Attribution used wait4 directly, and the previous
round used another observation method: compare wall only within a series.
Internal stage boundaries did not change. Console.finish occurs after stage
timing and was not changed.

strace observes owned processes and process/sleep/kill syscalls. The parser
records clone latency, cargo-to-product exec, readiness exit-to-reap lag,
retry, assertion core (first auth clone to last grep exit), adapter/outer-shell
exit-to-reap lag, signal-to-reaped-and-group-absent stop and poll counts.
Clone latency is not the whole OwnedProcess::spawn API. Raw timestamps are
observer-affected; sub-millisecond/occasionally slightly negative cross-process
lags are retained as trace noise, not physical negative latency.
Traced minus adjacent control stage paired medians were **A +90.5 ms,
B +49.5 ms**; do not use traced totals as untraced performance gains.

Main-thread CPU is `/proc/PID/schedstat` sampled while the supervisor is exited
but unreaped: it excludes its console thread and descendants. Its fields
are runtime, runqueue wait and timeslices ([Linux scheduler documentation](https://www.kernel.org/doc/html/latest/scheduler/sched-stats.html)).
wait4 user/system CPU includes accounted waited descendants; maxrss is a
largest-process high-water mark, **not simultaneous process-tree peak**
([getrusage documentation](https://man7.org/linux/man-pages/man2/getrusage.2.html)).
No memory reduction or reliable p99 claim. Statistics use inclusive quartiles
and raw median absolute deviation (MAD), without scaling.

## Untraced results

All 90 final runtime executions exited 0, recorded closed ports and no remaining
process using the fixed product executable. Full distributions and paired
differences: [final-runtime-summary.json](final-runtime-summary.json).
Individual values, commands and failure fields:
[final-runtime-raw.json](final-runtime-raw.json).

| Internal stage (ms) | n | median | IQR | MAD | min | max |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| A | 30 | 1536 | 1.75 | 1 | 1494 | 1576 |
| B | 30 | 1617 | 40 | 20.5 | 1557 | 1679 |
| C | 30 | 1528 | 25.75 | 9 | 1478 | 1649 |
| paired C−B | 30 | −71.5 | 67.5 | 41 | −154 | +10 |
| paired C−A | 30 | −7 | 32.25 | 8 | −56 | +115 |
| paired B−A | 30 | +99.5 | 40.75 | 35.5 | +22 | +143 |

C improves over B in 29/30 pairs and over A in 21/30. The original ~0.1 s
systematic regression is no longer present in the paired median, but 9 C/A
pairs are slower and the +115 ms maximum is retained. No zero-regression or
tail guarantee. Paired medians are not algebraically additive.

Whole-invocation wall medians A/B/C: 1583.956/1666.336/1575.831 ms.
Main-thread CPU medians: 6.430/7.572/7.076 ms; C−B paired median −0.420 ms,
23/30 lower (range −2.600 to +10.125); C−A +0.865 ms. Whole accounted CPU C−B
paired median −1.560 ms, 15 lower/15 higher, IQR 52.654 ms. No consistent CPU
growth versus B; this is not a universal CPU improvement claim.
Main-thread timeslice paired C−B median +1.5. Maxrss A/B/C medians
95614/95574/95582 KiB; this does not demonstrate a memory saving.

## Attribution and cost

Post-change traced B/C medians:

| Segment | B | C |
| --- | ---: | ---: |
| readiness exit observation, summed ms | 68.878 | 9.507 |
| readiness first clone to final reap, ms | 1086.024 | 1026.639 |
| retry gap, ms | 1001.208 | 1000.952 |
| assertion core, ms | 503.469 | 505.905 |
| adapter exit observation, ms | 21.781 | 13.570 |
| service stop, ms | 26.338 | 6.278 |
| readiness nonblocking wait calls | 4 | 5 |
| whole-stage root nonblocking wait calls | 53 | 55 |
| process-group existence probes | 2 | 2 |

Adapter polling code was unchanged; its median variation is not attributed
to an adapter optimization. Initial A real-auth stop median was 2.032 ms.
The separate older 1.5-second cleanup probe cannot be subtracted from A's
auth runtime. Full segments include spawn, retry and raw readiness count.

[builds.json](builds.json): fresh tool target B 7.480 s/C 7.370 s; no-op
B 66.842 ms/C 78.492 ms, each n=1. Cached dependencies, offline, no product
rebuild. Do not infer stable compiler gains. Debug C−B size +15,968 bytes
(+0.0744%); no new dependency. One-shot Chinese-directory compatibility wrapper
passed in 9.775 s including its fresh snapshot-local compile; this is a
functional check, not another paired runtime result.

## Validation, artifacts and audit

- Linux fmt/clippy, 4 unit tests, 28 acceptance tests pass (suite 73.153 s).
  Commands/results: `linux-validation.json` and `.txt`.
- Windows Rust 1.98.1, Python 3.14.7, Git Bash: fmt/clippy, 3 unit tests pass.
  Initial concurrent Windows/WSL test execution hit default port 18081 conflicts
  (8 failures including subtests); all logs retained. Later read-only inspection
  found no listener and bind succeeded; transient owner not identified.
  No process killed, assertion weakened or source changed. Full sequential
  repeat: 26 pass, 2 Unix-only skip (suite 119.707 s).
- Includes stubborn grandchildren/full grace, SIGINT/SIGTERM where supported,
  hung/slow readiness, >30 s readiness, occupied-port ownership, closed stdout,
  Unicode/space paths, caller PATH and real HTTP/cookie/redirect contracts.
- Windows optimization performance and native macOS were not measured.
  Full repository/browser/platform checks use exact final PR head CI;
  its links and terminal receipt are appended to PR #23 after upload.

Raw syscall files and every per-run JSON/output, including separate diagnostics,
warmups and compilation logs: `raw-traces-and-runs.tar.gz` with
`archive-sha256.txt`. These contain only local fixture credentials, never user
credentials. Snapshot service reports/fixtures and binaries remain at the
WSL target path above. Windows snapshot: `E:/gitclone/RSS-Reader/target/w3`.
No caches were deleted. Windows readable log copies normalize line endings and
trailing whitespace only; original byte streams remain in the Windows snapshot.

Audit saved summaries without rerunning services:

```bash
python3 docs/testing/evidence/2026-10-06-web-auth-polling-latency/recompute.py
# Optional: extract the archive into a fresh scratch directory, then:
python3 docs/testing/evidence/2026-10-06-web-auth-polling-latency/recompute.py \
  --traces /absolute/scratch/evidence
```

`measure_driver_attribution.py` and `measure_driver.py` preserve exact measurement
implementations; absolute paths identify original inputs. To reproduce execution,
prepare new commit snapshots/binaries and common product target, adapt BASE/ROOT
to a new experiment directory, then call `measure(label, series, pair, traced)`
in the orders above. Do not overwrite old series names/directories.
`parse_traces.py` and `summarize.py` preserve parser/statistical definitions.
Archive records include each actual command and allocated port.

The new WSL experiment occupies about 952 MB after the wrapper check; available
space then was 566.0 GB within WSL and 130.3 GB on E. This is separate from the
prior 1.9 GiB baseline/product build area and native Windows validation target.
