#!/usr/bin/env bash
# Compatibility entry: Rust owns service startup, readiness, cancellation and cleanup.
set -euo pipefail
profile="${1:?missing profile}"
web_port="${2:?missing web port}"
log_dir="${3:?missing log directory}"
case "$profile" in debug|release) ;; *) echo "expected debug or release profile" >&2; exit 1 ;; esac
script_path="${BASH_SOURCE[0]//\\//}"
script_dir="${script_path%/*}"
if [[ "$script_dir" == "$script_path" ]]; then script_dir=.; fi
exec "$BASH" "$script_dir/run_release_ui_regression.sh" \
  --web-auth-only "--$profile" --web-port "$web_port" --log-dir "$log_dir"
