#!/usr/bin/env bash
# Compatibility launcher only. Rust owns options, plan, state and exit codes.
set -euo pipefail
export PATH="/usr/bin:/bin:$PATH"
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"
if command -v cygpath >/dev/null 2>&1; then
  export RSSR_BASH="$(cygpath -w "$BASH")"
else
  export RSSR_BASH="$BASH"
fi
cargo build --quiet --locked --manifest-path scripts/release-ui/Cargo.toml \
  --target-dir target/release-ui-runner
exec target/release-ui-runner/debug/release-ui "$@"
