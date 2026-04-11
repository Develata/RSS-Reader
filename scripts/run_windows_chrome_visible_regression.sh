#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
cd "$repo_root"

profile="debug"
static_port="8112"
rssr_web_port="18098"
chrome_port="9225"
skip_build="false"
use_existing_servers="false"
keep_browser_open="false"
slow_ms="200"
log_dir=""
browser_check_dir="${CODEX_BROWSER_CHECK_DIR:-$HOME/codex-browser-check}"
username="smoke"
password="smoke-pass-123"
session_secret="rssr-web-browser-feed-smoke-session-secret-0123456789"

usage() {
  cat <<EOF
Usage: $0 [options]

Runs the Web SPA regression in a visible Windows Chrome window from WSL.

Options:
  --static-port PORT          Static SPA server port. Default: ${static_port}
  --rssr-web-port PORT        rssr-web server port. Default: ${rssr_web_port}
  --chrome-port PORT          Windows Chrome DevTools port. Default: ${chrome_port}
  --debug                     Use debug web build. Default.
  --release                   Use release web build.
  --skip-build                Reuse existing web build output.
  --use-existing-servers      Do not start local servers; only drive the browser.
  --keep-browser-open         Leave the regression tab open after success.
  --slow-ms MS                Delay between visible browser actions. Default: ${slow_ms}
  --log-dir DIR               Write logs and generated launch scripts under DIR.
  --browser-check-dir DIR     Windows Node working directory. Default: ${browser_check_dir}
  -h, --help                  Show this help.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --static-port)
      static_port="${2:?missing static port value}"
      shift 2
      ;;
    --rssr-web-port)
      rssr_web_port="${2:?missing rssr-web port value}"
      shift 2
      ;;
    --chrome-port)
      chrome_port="${2:?missing chrome port value}"
      shift 2
      ;;
    --debug)
      profile="debug"
      shift
      ;;
    --release)
      profile="release"
      shift
      ;;
    --skip-build)
      skip_build="true"
      shift
      ;;
    --use-existing-servers)
      use_existing_servers="true"
      shift
      ;;
    --keep-browser-open)
      keep_browser_open="true"
      shift
      ;;
    --slow-ms)
      slow_ms="${2:?missing slow-ms value}"
      shift 2
      ;;
    --log-dir)
      log_dir="${2:?missing log dir value}"
      shift 2
      ;;
    --browser-check-dir)
      browser_check_dir="${2:?missing browser check dir value}"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      usage >&2
      exit 1
      ;;
  esac
done

if [[ -z "$log_dir" ]]; then
  log_dir="target/windows-chrome-visible-regression/$(date +%Y%m%d-%H%M%S)"
fi

mkdir -p "$log_dir" "$browser_check_dir"

if ! command -v powershell.exe >/dev/null 2>&1; then
  echo "powershell.exe is required to launch Windows Chrome from WSL." >&2
  exit 1
fi

if ! command -v cmd.exe >/dev/null 2>&1; then
  echo "cmd.exe is required to run Windows Node from WSL." >&2
  exit 1
fi

runner_js="$repo_root/scripts/browser/rssr_visible_regression.mjs"
if [[ ! -f "$runner_js" ]]; then
  echo "Visible browser runner not found: ${runner_js}" >&2
  exit 1
fi

public_dir="target/dx/rssr-app/${profile}/web/public"
static_log="$log_dir/static-web.log"
rssr_web_log="$log_dir/rssr-web.log"
auth_state_file="$log_dir/rssr-web-auth.json"
chrome_pid_file="$log_dir/windows-chrome.pid"
chrome_pid_file_win="$(wslpath -w "$chrome_pid_file")"
launcher_ps1="$log_dir/launch-windows-chrome.ps1"
runner_ps1="$log_dir/run-visible-regression.ps1"
runner_js_win="$(wslpath -w "$runner_js")"
browser_check_dir_win="$(wslpath -w "$browser_check_dir")"
summary_file="$log_dir/summary.md"

static_pid=""
rssr_web_pid=""

cleanup() {
  if [[ -n "$static_pid" ]] && kill -0 "$static_pid" >/dev/null 2>&1; then
    kill "$static_pid" >/dev/null 2>&1 || true
    wait "$static_pid" >/dev/null 2>&1 || true
  fi

  if [[ -n "$rssr_web_pid" ]] && kill -0 "$rssr_web_pid" >/dev/null 2>&1; then
    kill "$rssr_web_pid" >/dev/null 2>&1 || true
    wait "$rssr_web_pid" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT INT TERM

wait_for_url() {
  local url="$1"
  local label="$2"

  for _ in {1..45}; do
    if curl --noproxy '*' -fsS "$url" >/dev/null 2>&1; then
      return 0
    fi
    sleep 1
  done

  echo "${label} did not become ready: ${url}" >&2
  return 1
}

if [[ "$use_existing_servers" != "true" ]]; then
  static_args=(--port "$static_port")
  if [[ "$profile" == "release" ]]; then
    static_args+=(--release)
  else
    static_args+=(--debug)
  fi
  if [[ "$skip_build" == "true" ]]; then
    static_args+=(--skip-build)
  fi

  bash scripts/run_web_spa_regression_server.sh "${static_args[@]}" >"$static_log" 2>&1 &
  static_pid=$!
  wait_for_url "http://127.0.0.1:${static_port}/entries" "Static SPA server"

  if [[ ! -d "$public_dir" ]]; then
    echo "Web build output not found: ${public_dir}" >&2
    exit 1
  fi

  RSS_READER_WEB_BIND="127.0.0.1:${rssr_web_port}" \
  RSS_READER_WEB_STATIC_DIR="$public_dir" \
  RSS_READER_WEB_USERNAME="$username" \
  RSS_READER_WEB_PASSWORD="$password" \
  RSS_READER_WEB_SESSION_SECRET="$session_secret" \
  RSS_READER_WEB_AUTH_STATE_FILE="$auth_state_file" \
  RSS_READER_WEB_ENABLE_SMOKE_HELPERS="true" \
  cargo run -p rssr-web >"$rssr_web_log" 2>&1 &
  rssr_web_pid=$!
  wait_for_url "http://127.0.0.1:${rssr_web_port}/healthz" "rssr-web server"
else
  wait_for_url "http://127.0.0.1:${static_port}/entries" "Existing Static SPA server"
  wait_for_url "http://127.0.0.1:${rssr_web_port}/healthz" "Existing rssr-web server"
fi

cat >"$launcher_ps1" <<EOF
\$ErrorActionPreference = "Stop"
\$port = ${chrome_port}
\$versionUrl = "http://127.0.0.1:\$port/json/version"

try {
  Invoke-WebRequest -UseBasicParsing -Uri \$versionUrl -TimeoutSec 2 | Out-Null
  Write-Host "Windows Chrome DevTools endpoint already ready on \$versionUrl"
  exit 0
} catch {
}

\$candidates = @(
  "\$env:ProgramFiles\\Google\\Chrome\\Application\\chrome.exe",
  "\${env:ProgramFiles(x86)}\\Google\\Chrome\\Application\\chrome.exe",
  "\$env:LocalAppData\\Google\\Chrome\\Application\\chrome.exe"
)

\$chrome = \$candidates | Where-Object { Test-Path \$_ } | Select-Object -First 1
if (-not \$chrome) {
  throw "Unable to locate Windows Chrome."
}

\$profileDir = Join-Path \$env:TEMP "rssr-codex-visible-profile-\$port"
New-Item -ItemType Directory -Force -Path \$profileDir | Out-Null

\$args = @(
  "--remote-debugging-port=\$port",
  "--remote-allow-origins=*",
  "--no-first-run",
  "--no-default-browser-check",
  "--user-data-dir=\$profileDir",
  "--new-window",
  "about:blank"
)

\$process = Start-Process -FilePath \$chrome -ArgumentList \$args -PassThru
\$process.Id | Out-File -Encoding ascii "${chrome_pid_file_win}"

for (\$i = 0; \$i -lt 45; \$i++) {
  try {
    Invoke-WebRequest -UseBasicParsing -Uri \$versionUrl -TimeoutSec 2 | Out-Null
    Write-Host "Windows Chrome DevTools endpoint ready on \$versionUrl"
    exit 0
  } catch {
    Start-Sleep -Seconds 1
  }
}

throw "Windows Chrome DevTools endpoint failed to become ready on \$versionUrl"
EOF

powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$(wslpath -w "$launcher_ps1")"

cat >"$runner_ps1" <<EOF
\$ErrorActionPreference = "Stop"
\$env:CDP_BASE = "http://127.0.0.1:${chrome_port}"
\$env:STATIC_BASE = "http://127.0.0.1:${static_port}"
\$env:RSSR_WEB_BASE = "http://127.0.0.1:${rssr_web_port}"
\$env:KEEP_BROWSER_OPEN = "${keep_browser_open}"
\$env:SLOW_MS = "${slow_ms}"
Set-Location "${browser_check_dir_win}"
node "${runner_js_win}"
if (\$LASTEXITCODE -ne 0) {
  exit \$LASTEXITCODE
}
EOF

powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$(wslpath -w "$runner_ps1")"

cat >"$summary_file" <<EOF
# Windows Chrome visible regression

- Date: $(date '+%Y-%m-%d %H:%M:%S %z')
- Commit: $(git rev-parse --short HEAD)
- Profile: ${profile}
- Static base: http://127.0.0.1:${static_port}
- rssr-web base: http://127.0.0.1:${rssr_web_port}
- Chrome CDP: http://127.0.0.1:${chrome_port}
- Browser check dir: ${browser_check_dir}
- Runner: ${runner_js}
- Keep browser open: ${keep_browser_open}

## Result

- static entries: pass
- static feeds: pass
- reader theme matrix: pass
- small viewport routes: pass
- rssr-web browser feed smoke: pass
EOF

echo "Windows Chrome visible regression passed"
echo "Summary: ${summary_file}"
