#!/usr/bin/env bash
set -euo pipefail

task_no_proxy="127.0.0.1,localhost,${no_proxy:-}"
export no_proxy="$task_no_proxy"
export NO_PROXY="$task_no_proxy"

profile="debug"
port="8091"
skip_build="false"
log_dir=""
chrome_bin="${CHROME_BIN:-google-chrome}"
node_bin="${NODE_BIN:-node}"
viewport="360,800"
preset=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --port)
      port="${2:?missing port value}"
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
    --log-dir)
      log_dir="${2:?missing log dir value}"
      shift 2
      ;;
    --username|--password)
      # 兼容旧调用；固定 smoke 的认证值由断言程序统一持有。
      shift 2
      ;;
    --chrome-bin)
      chrome_bin="${2:?missing chrome bin value}"
      shift 2
      ;;
    --viewport)
      viewport="${2:?missing viewport value}"
      shift 2
      ;;
    --preset)
      preset="${2:?missing preset value}"
      shift 2
      ;;
    *)
      echo "Usage: $0 [--port PORT] [--debug|--release] [--skip-build] [--log-dir DIR] [--chrome-bin BIN] [--viewport WIDTH,HEIGHT] [--preset PRESET]" >&2
      exit 1
      ;;
  esac
done

if [[ ! "$viewport" =~ ^([0-9]+),([0-9]+)$ ]]; then
  echo "Invalid viewport '${viewport}'; expected WIDTH,HEIGHT" >&2
  exit 1
fi
viewport_width="${BASH_REMATCH[1]}"
viewport_height="${BASH_REMATCH[2]}"

resolve_chrome_bin() {
  if command -v "$chrome_bin" >/dev/null 2>&1; then
    command -v "$chrome_bin"
    return
  fi

  local candidates=(
    "/mnt/c/Program Files/Google/Chrome/Application/chrome.exe"
    "/mnt/c/Program Files (x86)/Google/Chrome/Application/chrome.exe"
    "/c/Program Files/Google/Chrome/Application/chrome.exe"
    "/c/Program Files (x86)/Google/Chrome/Application/chrome.exe"
    "${LOCALAPPDATA:-}/Google/Chrome/Application/chrome.exe"
  )
  local candidate
  for candidate in "${candidates[@]}"; do
    if [[ -n "$candidate" && -x "$candidate" ]]; then
      printf '%s\n' "$candidate"
      return
    fi
  done

  echo "Chrome executable not found; pass --chrome-bin or set CHROME_BIN" >&2
  exit 1
}

chrome_bin="$(resolve_chrome_bin)"

resolve_node_bin() {
  if command -v "$node_bin" >/dev/null 2>&1; then
    command -v "$node_bin"
    return
  fi
  if command -v node.exe >/dev/null 2>&1; then
    command -v node.exe
    return
  fi

  echo "Node.js executable not found; set NODE_BIN" >&2
  exit 1
}

node_bin="$(resolve_node_bin)"

if [[ -z "$log_dir" ]]; then
  log_dir="target/static-web-small-viewport-smoke/$(date +%Y%m%d-%H%M%S)"
fi
mkdir -p "$log_dir"

server_log="$log_dir/static-web.log"
chrome_log="$log_dir/chrome.log"
summary_file="$log_dir/summary.md"
chrome_profile="$log_dir/chrome-profile"
cdp_port="$((port + 10000))"
chrome_profile_arg="$chrome_profile"
node_script_arg="scripts/browser/rssr_small_viewport_assertions.mjs"
node_artifact_dir="$log_dir"
windows_posix_shell="false"
to_windows_path() {
  if command -v wslpath >/dev/null 2>&1; then
    wslpath -w "$1"
  elif command -v cygpath >/dev/null 2>&1; then
    cygpath -w "$1"
  else
    printf '%s\n' "$1"
  fi
}
case "$(uname -s)" in
  CYGWIN*|MINGW*|MSYS*) windows_posix_shell="true" ;;
esac
if [[ "$chrome_bin" == *.exe ]]; then
  chrome_profile_arg="$(to_windows_path "$(realpath -m "$chrome_profile")")"
fi
if [[ "$node_bin" == *.exe || "$windows_posix_shell" == "true" ]]; then
  node_script_arg="$(to_windows_path "$(realpath -m "$node_script_arg")")"
  node_artifact_dir="$(to_windows_path "$(realpath -m "$node_artifact_dir")")"
fi

server_args=(--port "$port")
if [[ "$profile" == "release" ]]; then
  server_args+=(--release)
else
  server_args+=(--debug)
fi
if [[ "$skip_build" == "true" ]]; then
  server_args+=(--skip-build)
fi

bash scripts/run_web_spa_regression_server.sh "${server_args[@]}" >"$server_log" 2>&1 &
server_pid=$!
chrome_pid=""

cleanup() {
  if [[ -n "$chrome_pid" ]] && kill -0 "$chrome_pid" >/dev/null 2>&1; then
    kill "$chrome_pid" >/dev/null 2>&1 || true
    wait "$chrome_pid" >/dev/null 2>&1 || true
  fi
  if kill -0 "$server_pid" >/dev/null 2>&1; then
    kill "$server_pid" >/dev/null 2>&1 || true
    wait "$server_pid" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT INT TERM

ready="false"
for _ in {1..60}; do
  if curl -fsS "http://127.0.0.1:${port}/entries" >/dev/null 2>&1; then
    ready="true"
    break
  fi
  sleep 1
done
if [[ "$ready" != "true" ]]; then
  echo "Static web server failed to become ready on http://127.0.0.1:${port}" >&2
  echo "See log: ${server_log}" >&2
  exit 1
fi

"$chrome_bin" \
  --headless=new \
  --disable-gpu \
  --disable-background-networking \
  --no-first-run \
  --no-default-browser-check \
  --no-proxy-server \
  --no-sandbox \
  --remote-allow-origins='*' \
  --remote-debugging-port="$cdp_port" \
  --user-data-dir="$chrome_profile_arg" \
  about:blank >"$chrome_log" 2>&1 &
chrome_pid=$!

cdp_ready="false"
for _ in {1..30}; do
  if curl -fsS "http://127.0.0.1:${cdp_port}/json/version" >/dev/null 2>&1; then
    cdp_ready="true"
    break
  fi
  sleep 1
done
if [[ "$cdp_ready" != "true" ]]; then
  echo "Chrome CDP failed to become ready on port ${cdp_port}" >&2
  echo "See log: ${chrome_log}" >&2
  exit 1
fi

if ! "$node_bin" "$node_script_arg" \
  --cdp-base "http://127.0.0.1:${cdp_port}" \
  --static-base "http://127.0.0.1:${port}" \
  --artifact-dir "$node_artifact_dir" \
  --width "$viewport_width" \
  --height "$viewport_height" \
  --dpr "3" \
  --preset "$preset"; then
  cat >"$summary_file" <<EOF
# Static Web 小视口 Smoke

- commit：$(git rev-parse --short HEAD)
- profile：${profile}
- viewport：${viewport_width}×${viewport_height} @ DPR 3
- fixtures：mobile-ui-overflow、mobile-ui-short
- chrome：${chrome_bin}
- 结果：失败
- 断言：${log_dir}/assertions.json
- 服务器日志：${server_log}
- Chrome 日志：${chrome_log}
EOF
  exit 1
fi

cat >"$summary_file" <<EOF
# Static Web 小视口 Smoke

- 日期：$(date '+%Y-%m-%d %H:%M:%S %z')
- commit：$(git rev-parse --short HEAD)
- profile：${profile}
- viewport：${viewport_width}×${viewport_height} @ DPR 3
- fixtures：mobile-ui-overflow、mobile-ui-short
- preset：${preset:-default}
- chrome：${chrome_bin}
- 结果：通过

## 产物

- assertions.json
- entries / feeds / settings / reader 的 HTML 与 PNG
- entries-short-directory、entries-desktop 的 HTML 与 PNG
- static-web.log、chrome.log
EOF

echo "static web small viewport smoke passed"
echo "Summary: ${summary_file}"
echo "Artifacts: ${log_dir}"
