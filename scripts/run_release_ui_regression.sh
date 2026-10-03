#!/usr/bin/env bash
# Compatibility launcher only. Rust owns options, plan, state and exit codes.
set -euo pipefail
script_path="${BASH_SOURCE[0]}"
case "$OSTYPE" in
  msys*|cygwin*)
    # PowerShell may omit Git Bash tools; never move them ahead of caller tools.
    export PATH="$PATH:/usr/bin:/bin"
    export RSSR_BASH="${RSSR_BASH:-$(/usr/bin/cygpath -w "$BASH")}"
    script_path="${script_path//\\//}"
    ;;
  *) export RSSR_BASH="${RSSR_BASH:-$BASH}" ;;
esac
script_dir="${script_path%/*}"
if [[ "$script_dir" == "$script_path" ]]; then script_dir=.; fi
repo_root="$(cd "$script_dir/.." && pwd)"
cd "$repo_root"
# This is a host tool even when the caller configures a product cross target.
host=""
while IFS=' ' read -r key value; do
  if [[ "$key" == host: ]]; then host="$value"; fi
done <<< "$(rustc -vV)"
if [[ -z "$host" ]]; then echo 'Could not resolve rustc host target' >&2; exit 1; fi
cargo build --quiet --locked --manifest-path scripts/release-ui/Cargo.toml \
  --target "$host" --target-dir target/release-ui-runner
exec "target/release-ui-runner/$host/debug/release-ui" "$@"
