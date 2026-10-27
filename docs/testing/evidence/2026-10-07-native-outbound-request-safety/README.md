# Native outbound request safety evidence

Workdir: `E:/gitclone/RSS-Reader/`, Windows `LAPTOP-H6JEOCF0`, Rust 1.98.1.
Base: `88b6abb32b992c7b19bb81ff5718ebb4cd8a6be1`. Implementation commit: `0bad743876b1e150680b4aa43f1d34c49623c378`.

- `baseline-regressions.txt`: test-first run against unchanged base production code,
  exit 101, 3 passed and 9 expected failures. All credentials, signatures and
  configuration payloads in this log are synthetic loopback fixtures.
- `targeted-tests.txt`: corrected infra unit tests (48 passed, 1 existing ignored),
  14 outbound request regressions, and the original configuration roundtrip.
- `clippy-final.txt`: workspace / all-targets Clippy with warnings denied.
- `workspace-test-summary.txt`: complete workspace suite results, including stale
  hash and generation writeback protection (354 passed, 0 failed, 2 pre-existing
  manual performance probes ignored).
- `tested-source.json`: SHA-256 of each source/test file used for local checks.
  Tests ran before the source commit, so the aggregate runner's `commit` field
  names the base HEAD with these working-tree modifications. It is not evidence
  that the clean base passed the new safety assertions.

The handoff records the validation matrix and compatibility boundaries. Full
local logs remain in `target/outbound-security/`. No performance comparison was
run; build/test durations are validation metadata only. HTTPS downgrade, default
ports, and IPv6 normalization have pure policy coverage; loopback request capture
uses HTTP and does not claim a TLS integration or device test.

The checked-in evidence is the local pre-push checkpoint. Final remote head and
CI results are recorded on the Draft PR without producing another CI run solely
to update this checkpoint.
