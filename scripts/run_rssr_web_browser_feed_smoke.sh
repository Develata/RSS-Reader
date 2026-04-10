#!/usr/bin/env bash
set -euo pipefail

profile="debug"
port="18088"
skip_build="false"
log_dir=""
username="smoke"
password="smoke-pass-123"
session_secret="rssr-web-browser-feed-smoke-session-secret-0123456789"

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
    --session-secret)
      session_secret="${2:?missing session secret value}"
      shift 2
      ;;
    *)
      echo "Usage: $0 [--port PORT] [--debug|--release] [--skip-build] [--log-dir DIR] [--username USER] [--password PASS] [--session-secret SECRET]" >&2
      exit 1
      ;;
  esac
done

if [[ -z "$log_dir" ]]; then
  log_dir="target/rssr-web-browser-feed-smoke/$(date +%Y%m%d-%H%M%S)"
fi

mkdir -p "$log_dir"

public_dir="target/dx/rssr-app/${profile}/web/public"
auth_state_file="$log_dir/rssr-web-auth.json"
log_file="$log_dir/rssr-web.log"
chrome_log="$log_dir/chrome.log"
dom_file="$log_dir/browser-feed-smoke.html"
screenshot_file="$log_dir/browser-feed-smoke.png"
summary_file="$log_dir/summary.md"

if [[ "$skip_build" != "true" ]]; then
  echo "Building rssr-app web bundle (${profile})..."
  if [[ "$profile" == "release" ]]; then
    dx build --platform web --package rssr-app --release >/dev/null
  else
    dx build --platform web --package rssr-app >/dev/null
  fi
fi

if [[ ! -d "$public_dir" ]]; then
  echo "Web build output not found: $public_dir" >&2
  exit 1
fi

cat >"$summary_file" <<EOF
# rssr-web 浏览器自动 feed smoke

- 日期：$(date '+%Y-%m-%d %H:%M:%S %z')
- commit：$(git rev-parse --short HEAD)
- profile：${profile}
- bind：http://127.0.0.1:${port}
- 用户名：${username}
- 密码：${password}
- DOM：${dom_file}
- 截图：${screenshot_file}
- 日志：${log_file}
- Chrome 日志：${chrome_log}

## 固定动作

1. 打开 \`/__codex/browser-feed-smoke\`
2. 自动完成：
   - 建立登录态
   - 打开 \`/feeds\`
   - 填入同源 \`/__codex/feed-fixture.xml\`
   - 点击 \`data-action="add-feed"\`
   - 点击 \`data-action="refresh-feed"\`
   - 点击 \`data-nav="feed-entries"\`
   - 进入订阅文章页并确认 \`Codex Smoke Entry\`

## 结果

- data-result：
- feed fixture：
- 最终路径：
- 文章标题：
- 是否通过：
EOF

RSS_READER_WEB_BIND="127.0.0.1:${port}" \
RSS_READER_WEB_STATIC_DIR="$public_dir" \
RSS_READER_WEB_USERNAME="$username" \
RSS_READER_WEB_PASSWORD="$password" \
RSS_READER_WEB_SESSION_SECRET="$session_secret" \
RSS_READER_WEB_AUTH_STATE_FILE="$auth_state_file" \
RSS_READER_WEB_ENABLE_SMOKE_HELPERS="true" \
cargo run -p rssr-web >"$log_file" 2>&1 &
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
  if curl -fsS "http://127.0.0.1:${port}/healthz" >/dev/null 2>&1; then
    ready="true"
    break
  fi
  sleep 1
done

if [[ "$ready" != "true" ]]; then
  echo "rssr-web browser feed smoke server failed to become ready on http://127.0.0.1:${port}" >&2
  echo "See log: ${log_file}" >&2
  exit 1
fi

helper_url="http://127.0.0.1:${port}/__codex/browser-feed-smoke"

google-chrome \
  --headless=new \
  --no-sandbox \
  --disable-dev-shm-usage \
  --disable-gpu \
  --run-all-compositor-stages-before-draw \
  --virtual-time-budget=120000 \
  --window-size=1440,1200 \
  --dump-dom \
  "$helper_url" >"$dom_file" 2>"$chrome_log"

google-chrome \
  --headless=new \
  --no-sandbox \
  --disable-dev-shm-usage \
  --disable-gpu \
  --run-all-compositor-stages-before-draw \
  --virtual-time-budget=120000 \
  --window-size=1440,1200 \
  --screenshot="$screenshot_file" \
  "$helper_url" >>"$chrome_log" 2>&1

grep -q 'data-smoke="rssr-web-browser-feed-smoke"' "$dom_file"
grep -q 'data-result="pass"' "$dom_file"
grep -q 'Codex Smoke Feed' "$dom_file"
grep -q 'Codex Smoke Entry' "$dom_file"
grep -q '"/feeds/' "$dom_file"

echo "rssr-web browser feed smoke passed"
echo "DOM: $dom_file"
echo "Screenshot: $screenshot_file"
echo "Summary template: $summary_file"
