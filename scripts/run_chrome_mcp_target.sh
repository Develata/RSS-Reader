#!/usr/bin/env bash
set -euo pipefail

port="9222"
profile_dir="target/chrome-mcp-profile"
start_url="about:blank"
headless="true"
restart="false"
chrome_bin="${CHROME_BIN:-}"

usage() {
  cat <<EOF
Usage: $0 [--port PORT] [--profile-dir DIR] [--url URL] [--chrome-bin PATH] [--headed] [--restart]

Starts a Chrome for Testing instance suitable for chrome-devtools-mcp and waits
until the DevTools endpoint is reachable.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --port)
      port="${2:?missing port value}"
      shift 2
      ;;
    --profile-dir)
      profile_dir="${2:?missing profile dir value}"
      shift 2
      ;;
    --url)
      start_url="${2:?missing start url value}"
      shift 2
      ;;
    --chrome-bin)
      chrome_bin="${2:?missing chrome bin value}"
      shift 2
      ;;
    --headed)
      headless="false"
      shift
      ;;
    --restart)
      restart="true"
      shift
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

resolve_chrome_bin() {
  if [[ -n "$chrome_bin" ]]; then
    printf '%s\n' "$chrome_bin"
    return 0
  fi

  if command -v google-chrome >/dev/null 2>&1; then
    command -v google-chrome
    return 0
  fi

  if [[ -x "/home/develata/.local/opt/chrome-for-testing/chrome" ]]; then
    printf '%s\n' "/home/develata/.local/opt/chrome-for-testing/chrome"
    return 0
  fi

  echo "Unable to locate Chrome for Testing. Set CHROME_BIN or use --chrome-bin." >&2
  exit 1
}

chrome_bin="$(resolve_chrome_bin)"
mkdir -p "$profile_dir"

pid_file="${profile_dir}/chrome-mcp.pid"
log_file="${profile_dir}/chrome-mcp.log"
version_url="http://127.0.0.1:${port}/json/version"

if [[ "$restart" == "true" && -f "$pid_file" ]]; then
  old_pid="$(cat "$pid_file" || true)"
  if [[ -n "$old_pid" ]] && kill -0 "$old_pid" >/dev/null 2>&1; then
    kill "$old_pid" >/dev/null 2>&1 || true
    wait "$old_pid" >/dev/null 2>&1 || true
  fi
  rm -f "$pid_file"
fi

if curl -fsS "$version_url" >/dev/null 2>&1; then
  echo "Chrome MCP target already ready on ${version_url}"
  curl -fsS "$version_url"
  exit 0
fi

if [[ "$restart" != "true" ]] && [[ -f "$pid_file" ]]; then
  old_pid="$(cat "$pid_file" || true)"
  if [[ -n "$old_pid" ]] && kill -0 "$old_pid" >/dev/null 2>&1; then
    echo "Existing chrome-mcp target pid ${old_pid} is still running but ${version_url} is not reachable." >&2
    echo "Retry with --restart or remove ${pid_file}." >&2
    exit 1
  fi
fi

rm -rf "$profile_dir/Default" "$profile_dir/SingletonLock" "$profile_dir/SingletonSocket" "$profile_dir/SingletonCookie"

chrome_args=(
  --remote-debugging-port="${port}"
  --remote-allow-origins=*
  --user-data-dir="$(realpath "$profile_dir")"
  --no-sandbox
  --disable-dev-shm-usage
  --disable-gpu
)

if [[ "$headless" == "true" ]]; then
  chrome_args+=(--headless=new)
fi

nohup "$chrome_bin" "${chrome_args[@]}" "$start_url" >"$log_file" 2>&1 &
chrome_pid=$!
echo "$chrome_pid" >"$pid_file"

ready="false"
for _ in {1..30}; do
  if curl -fsS "$version_url" >/dev/null 2>&1; then
    ready="true"
    break
  fi

  if ! kill -0 "$chrome_pid" >/dev/null 2>&1; then
    break
  fi

  sleep 1
done

if [[ "$ready" != "true" ]]; then
  echo "Chrome MCP target failed to become ready on ${version_url}" >&2
  echo "Log: ${log_file}" >&2
  exit 1
fi

echo "Chrome MCP target is ready."
echo "Chrome: ${chrome_bin}"
echo "Port: ${port}"
echo "Profile: ${profile_dir}"
echo "Log: ${log_file}"
echo "PID: ${chrome_pid}"
curl -fsS "$version_url"
