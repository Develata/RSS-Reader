#!/usr/bin/env bash
set -euo pipefail

profile="debug"
port="8091"
skip_build="false"
log_dir=""
username="smoke"
password="smoke-pass-123"
next_path="/entries/2"
seed="reader-demo"
chrome_bin="${CHROME_BIN:-google-chrome}"

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
    --next)
      next_path="${2:?missing next value}"
      shift 2
      ;;
    --chrome-bin)
      chrome_bin="${2:?missing chrome bin value}"
      shift 2
      ;;
    *)
      echo "Usage: $0 [--port PORT] [--debug|--release] [--skip-build] [--log-dir DIR] [--username USER] [--password PASS] [--next PATH] [--chrome-bin BIN]" >&2
      exit 1
      ;;
  esac
done

if [[ -z "$log_dir" ]]; then
  log_dir="target/static-web-reader-theme-matrix/$(date +%Y%m%d-%H%M%S)"
fi

mkdir -p "$log_dir"

server_log="$log_dir/static-web.log"
summary_file="$log_dir/summary.md"

cat >"$summary_file" <<EOF
# Static Web /reader 主题矩阵 Smoke

- 日期：$(date '+%Y-%m-%d %H:%M:%S %z')
- commit：$(git rev-parse --short HEAD)
- profile：${profile}
- bind：http://127.0.0.1:${port}
- helper next：${next_path}
- seed：${seed}
- chrome：${chrome_bin}
- 服务器日志：${server_log}

## 主题结果

- 默认主题：
- Atlas Sidebar：
- Newsprint：
- Amethyst Glass：
- Midnight Ledger：

## 产物

- default.html
- atlas-sidebar.html
- newsprint.html
- forest-desk.html
- midnight-ledger.html
- *.png

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
  echo "Static web theme matrix server failed to become ready on http://127.0.0.1:${port}" >&2
  echo "See log: ${server_log}" >&2
  exit 1
fi

theme_keys=("none" "atlas-sidebar" "newsprint" "forest-desk" "midnight-ledger")
theme_labels=("默认主题" "Atlas Sidebar" "Newsprint" "Amethyst Glass" "Midnight Ledger")
theme_markers=("" "#b24c3d" "#8f3f2b" "#8b5cf6" "#53c0bb")
theme_slugs=("default" "atlas-sidebar" "newsprint" "forest-desk" "midnight-ledger")

run_theme_check() {
  local theme_key="$1"
  local theme_label="$2"
  local theme_marker="$3"
  local slug="$4"
  local helper_url="http://127.0.0.1:${port}/__codex/setup-local-auth?username=${username}&password=${password}&seed=${seed}&next=${next_path}"
  local html_file="$log_dir/${slug}.html"
  local screenshot_file="$log_dir/${slug}.png"

  if [[ "$theme_key" != "none" ]]; then
    helper_url="${helper_url}&preset=${theme_key}"
  fi

  "$chrome_bin" \
    --headless=new \
    --disable-gpu \
    --no-sandbox \
    --window-size=1440,1200 \
    --virtual-time-budget=8000 \
    --screenshot="$screenshot_file" \
    "$helper_url" >/dev/null 2>&1

  "$chrome_bin" \
    --headless=new \
    --disable-gpu \
    --no-sandbox \
    --window-size=1440,1200 \
    --virtual-time-budget=8000 \
    --dump-dom \
    "$helper_url" >"$html_file"

  rg -q 'data-page="reader"' "$html_file"
  rg -q 'data-layout="reader-page"' "$html_file"
  rg -q 'Demo Entry Two' "$html_file"

  if [[ "$theme_key" == "none" ]]; then
    if rg -q 'id="user-custom-css"' "$html_file"; then
      echo "${theme_label}: unexpected user-custom-css style tag in default theme" >&2
      exit 1
    fi
  else
    rg -q 'id="user-custom-css"' "$html_file"
    rg -q --fixed-strings "$theme_marker" "$html_file"
  fi
}

for i in "${!theme_keys[@]}"; do
  run_theme_check "${theme_keys[$i]}" "${theme_labels[$i]}" "${theme_markers[$i]}" "${theme_slugs[$i]}"
done

echo "static web /reader theme matrix passed"
echo "Summary template: ${summary_file}"
echo "Artifacts: ${log_dir}"
