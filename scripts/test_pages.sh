#!/usr/bin/env bash
set -euo pipefail
public="${PAGES_PUBLIC_DIR:-target/dx/rssr-app/release/web/public}"
base="${PAGES_BASE_PATH-/RSS-Reader}"
log_root="${PAGES_SMOKE_LOG_DIR:-target/pages-smoke}"
mkdir -p "$log_root"
log_dir="$(mktemp -d "$log_root/run.XXXXXX")"
profile_dir="$(mktemp -d)"
server_pid=""
chrome_pid=""

cleanup_profile_best_effort() {
  local attempt
  for attempt in {1..10}; do
    if rm -rf -- "$profile_dir" 2>/dev/null; then
      return 0
    fi
    sleep 0.1
  done
  echo "::warning::Failed to remove temporary Chrome profile after retries: $profile_dir" >&2
  return 0
}

cleanup() {
  result=$?
  trap - EXIT
  for pid in "$chrome_pid" "$server_pid"; do
    if [[ -n "$pid" ]]; then
      kill "$pid" 2>/dev/null || true
      wait "$pid" 2>/dev/null || true
    fi
  done
  # Chrome children can briefly recreate profile files after the parent exits. Cleanup must never
  # turn an otherwise successful smoke test into a failure; retry the race and then degrade to a
  # warning so the runner can reclaim /tmp.
  cleanup_profile_best_effort
  if (( result != 0 )); then
    echo "Pages smoke failed; diagnostics: $log_dir" >&2
    tail -n 60 "$log_dir/server.log" "$log_dir/chrome.log" >&2 || true
  fi
  exit "$result"
}
trap cleanup EXIT
python3 scripts/prepare_pages.py "$public" --serve --base-path "$base" \
  --port 0 --port-file "$log_dir/server.port" >"$log_dir/server.log" 2>&1 &
server_pid=$!
google-chrome --headless --no-sandbox --disable-gpu --remote-debugging-port=0 \
  --user-data-dir="$profile_dir" about:blank >"$log_dir/chrome.log" 2>&1 &
chrome_pid=$!
ready=false
for _ in {1..150}; do
  kill -0 "$server_pid" "$chrome_pid" 2>/dev/null || break
  if [[ -s "$log_dir/server.port" && -s "$profile_dir/DevToolsActivePort" ]]; then
    read -r server_port < "$log_dir/server.port"
    chrome_port="$(head -n1 "$profile_dir/DevToolsActivePort")"
    cdp_base="http://127.0.0.1:$chrome_port"
    test_base="http://127.0.0.1:$server_port${base%/}"
    if curl -fsS --max-time 1 "$cdp_base/json/version" >/dev/null 2>&1 \
      && curl -fsS --max-time 1 "$test_base/" >/dev/null 2>&1; then
      ready=true
      break
    fi
  fi
  sleep 0.2
done
if [[ "$ready" != true ]]; then
  echo '::error::Pages test server or Chrome failed to start.'
  exit 1
fi
CDP_BASE="$cdp_base" PAGES_TEST_BASE="$test_base" PAGES_SMOKE_LOG_DIR="$log_dir" \
  timeout 120s node scripts/browser/pages_smoke.mjs
