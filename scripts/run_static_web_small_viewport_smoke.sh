#!/usr/bin/env bash
set -euo pipefail

profile="debug"
port="8091"
skip_build="false"
log_dir=""
username="smoke"
password="smoke-pass-123"
seed="reader-demo"
chrome_bin="${CHROME_BIN:-google-chrome}"
viewport="390,844"
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
    --username)
      username="${2:?missing username value}"
      shift 2
      ;;
    --password)
      password="${2:?missing password value}"
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
      echo "Usage: $0 [--port PORT] [--debug|--release] [--skip-build] [--log-dir DIR] [--username USER] [--password PASS] [--chrome-bin BIN] [--viewport WIDTH,HEIGHT] [--preset PRESET]" >&2
      exit 1
      ;;
  esac
done

if [[ -z "$log_dir" ]]; then
  log_dir="target/static-web-small-viewport-smoke/$(date +%Y%m%d-%H%M%S)"
fi

mkdir -p "$log_dir"

server_log="$log_dir/static-web.log"
summary_file="$log_dir/summary.md"

cat >"$summary_file" <<EOF
# Static Web 小视口 Smoke

- 日期：$(date '+%Y-%m-%d %H:%M:%S %z')
- commit：$(git rev-parse --short HEAD)
- profile：${profile}
- bind：http://127.0.0.1:${port}
- viewport：${viewport}
- seed：${seed}
- preset：${preset:-default}
- chrome：${chrome_bin}
- 服务器日志：${server_log}

## 路径结果

- /entries：
- /feeds：
- /settings：
- /entries/2：

## 产物

- entries.html / entries.png
- feeds.html / feeds.png
- settings.html / settings.png
- reader.html / reader.png

## 是否通过

- 
EOF

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

cleanup() {
  if kill -0 "$server_pid" >/dev/null 2>&1; then
    kill "$server_pid" >/dev/null 2>&1 || true
    wait "$server_pid" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT INT TERM

ready="false"
for _ in {1..30}; do
  if curl -fsS "http://127.0.0.1:${port}/entries" >/dev/null 2>&1; then
    ready="true"
    break
  fi
  sleep 1
done

if [[ "$ready" != "true" ]]; then
  echo "Static web small viewport server failed to become ready on http://127.0.0.1:${port}" >&2
  echo "See log: ${server_log}" >&2
  exit 1
fi

route_slugs=("entries" "feeds" "settings" "reader")
route_paths=("/entries" "/feeds" "/settings" "/entries/2")
route_pages=("entries" "feeds" "settings" "reader")
route_markers=('data-layout="entries-layout"' 'data-page="feeds"' 'data-layout="settings-grid"' 'Demo Entry Two')

run_route_check() {
  local slug="$1"
  local next_path="$2"
  local expected_page="$3"
  local expected_marker="$4"
  local helper_url="http://127.0.0.1:${port}/__codex/setup-local-auth?username=${username}&password=${password}&seed=${seed}&next=${next_path}"
  local html_file="$log_dir/${slug}.html"
  local screenshot_file="$log_dir/${slug}.png"

  if [[ -n "$preset" ]]; then
    helper_url="${helper_url}&preset=${preset}"
  fi

  "$chrome_bin" \
    --headless=new \
    --disable-gpu \
    --no-sandbox \
    --window-size="${viewport}" \
    --virtual-time-budget=8000 \
    --screenshot="$screenshot_file" \
    "$helper_url" >/dev/null 2>&1

  "$chrome_bin" \
    --headless=new \
    --disable-gpu \
    --no-sandbox \
    --window-size="${viewport}" \
    --virtual-time-budget=8000 \
    --dump-dom \
    "$helper_url" >"$html_file"

  rg -q "data-page=\"${expected_page}\"" "$html_file"
  rg -q --fixed-strings "$expected_marker" "$html_file"
}

for i in "${!route_slugs[@]}"; do
  run_route_check "${route_slugs[$i]}" "${route_paths[$i]}" "${route_pages[$i]}" "${route_markers[$i]}"
done

echo "static web small viewport smoke passed"
echo "Summary template: ${summary_file}"
echo "Artifacts: ${log_dir}"
