#!/usr/bin/env bash
set -euo pipefail

profile="debug"
port="18081"
skip_build="false"
log_dir=""
username="smoke"
password="smoke-pass-123"
session_secret="rssr-web-browser-smoke-session-secret-0123456789"
feed_url="https://www.ruanyifeng.com/blog/atom.xml"

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
    --feed-url)
      feed_url="${2:?missing feed url value}"
      shift 2
      ;;
    *)
      echo "Usage: $0 [--port PORT] [--debug|--release] [--skip-build] [--log-dir DIR] [--username USER] [--password PASS] [--session-secret SECRET] [--feed-url URL]" >&2
      exit 1
      ;;
  esac
done

if [[ -z "$log_dir" ]]; then
  log_dir="target/rssr-web-browser-smoke/$(date +%Y%m%d-%H%M%S)"
fi

mkdir -p "$log_dir"

public_dir="target/dx/rssr-app/${profile}/web/public"
auth_state_file="$log_dir/rssr-web-auth.json"
log_file="$log_dir/rssr-web.log"
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
# rssr-web 浏览器手工 smoke

- 日期：$(date '+%Y-%m-%d %H:%M:%S %z')
- commit：$(git rev-parse --short HEAD)
- profile：${profile}
- bind：http://127.0.0.1:${port}
- 用户名：${username}
- 密码：${password}
- 推荐 feed：${feed_url}
- 日志：${log_file}

## 固定手工步骤

1. 打开 \`/login\`，用上面的用户名和密码登录。
2. 进入 \`/feeds\`。
3. 在 \`data-field="feed-url-input"\` 输入推荐 feed：

   \`${feed_url}\`

4. 点击 \`data-action="add-feed"\`。
5. 确认页面出现新的 feed 卡片，且卡片标题链接带有 \`data-nav="feed-entries"\`。
6. 点击该卡片上的 \`data-action="refresh-feed"\`。
7. 如果页面出现文章，点击 `data-nav="feed-entries"` 进入文章页；如能进入阅读页，再补看 `/reader`。
8. 打开 \`/settings\`，确认登录态下设置页可达。
9. 访问 \`/logout\`，确认会回到 \`/login\`。

## 固定 selector / 期望

- 登录页：
- \`data-field="feed-url-input"\`：
- \`data-action="add-feed"\`：
- \`data-action="refresh-feed"\`：
- \`data-nav="feed-entries"\`：
- \`/settings\`：
- \`/logout\`：

## 结果补充

- 新 feed 是否出现：
- 首次刷新是否成功：
- `/entries` 或 `/reader`：
- 代理 feed：
- console：
- 是否通过：

## 说明

- 这条 smoke 当前仍是手工项，原因不是 selector 不稳定，而是当前仓库环境里的 Chrome MCP / DevTools 连接不稳定，尚未形成可重复的浏览器端 UI 自动化。
EOF

RSS_READER_WEB_BIND="127.0.0.1:${port}" \
RSS_READER_WEB_STATIC_DIR="$public_dir" \
RSS_READER_WEB_USERNAME="$username" \
RSS_READER_WEB_PASSWORD="$password" \
RSS_READER_WEB_SESSION_SECRET="$session_secret" \
RSS_READER_WEB_AUTH_STATE_FILE="$auth_state_file" \
cargo run -p rssr-web 2>&1 | tee "$log_file" &
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
  echo "rssr-web browser smoke server failed to become ready on http://127.0.0.1:${port}" >&2
  echo "See log: ${log_file}" >&2
  exit 1
fi

echo "rssr-web browser smoke server is ready on http://127.0.0.1:${port}"
echo "Username: ${username}"
echo "Password: ${password}"
echo "Recommended feed URL: ${feed_url}"
echo "Summary template: ${summary_file}"
echo "Log file: ${log_file}"
echo

wait "$server_pid"
