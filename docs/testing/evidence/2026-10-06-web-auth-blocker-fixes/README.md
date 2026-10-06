# 2026-10-06 web-auth blocker fixes evidence

Fixed A: f69087a61a4627cbd3ba88fa082d7722df811dc5.
Fixed B: e6915a65335701fc8cbedec2dcfdda123c6c598f.

- paired-raw.json: all warmups and 20 balanced AB/BA pairs per command class,
  hashes, toolchain, build records and exact commands. The internal cleanup and
  web-auth rows derive from the same executions; they are not independent runs.
- paired-summary.json: n, median, IQR, MAD, min/max, paired differences and failures.
- measure.py: exact one-off WSL script used, not a general-purpose benchmark tool.
  For a repeat, check out the fixed candidate and set BASE to a fresh directory on
  the same WSL filesystem. Existing source snapshots fail closed; do not remove
  previous evidence or merge runs.
- builds.json: clean tool/product and measurement-probe builds, separate from runtime.
  Wrapper cold builds are retained in paired-raw.json warmups.
- linux-acceptance.json and windows-native-validation.json: native verification commands,
  wall times and outcomes; full logs remain under ignored target directories.
- Windows guard logs: old adapter fails 23 on non-ASCII filename arguments,
  new adapter completes the real HTTP contract. This models the CI limitation;
  it does not assert that the local curl build/code page equals the old CI runner.
- real-compatibility.json: real product through the three-argument compatibility entry.
- environment.json / windows-environment-final.json: platform, tools and unchanged
  Windows checkout status.

The unconditional cleanup wait is removed, but all 20 web-auth pairs remain slower
than A (median paired difference +98.5 ms). The overall performance gate has not
been approved. No p99, memory, full-browser or cross-platform speedup claim.
Final PR-head CI receipt is retained locally as evidence/ci-final.json and linked
in PR #23; historical heads are not substituted for final-head checks.
