#!/usr/bin/env bash
set -euo pipefail

profile="debug"
port="8091"
skip_build="false"
log_dir=""
username="smoke"
password="smoke-pass-123"
next_path="/entries"
seed=""

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
    --seed)
      seed="${2:?missing seed value}"
      shift 2
      ;;
    *)
      echo "Usage: $0 [--port PORT] [--debug|--release] [--skip-build] [--log-dir DIR] [--username USER] [--password PASS] [--next PATH] [--seed reader-demo]" >&2
      exit 1
      ;;
  esac
done

if [[ -z "$log_dir" ]]; then
  log_dir="target/static-web-browser-smoke/$(date +%Y%m%d-%H%M%S)"
fi

mkdir -p "$log_dir"

log_file="$log_dir/static-web.log"
summary_file="$log_dir/summary.md"
helper_url="http://127.0.0.1:${port}/__codex/setup-local-auth?username=${username}&password=${password}&next=${next_path}"
if [[ -n "$seed" ]]; then
  helper_url="${helper_url}&seed=${seed}"
fi

cat >"$summary_file" <<EOF
# Static Web 浏览器手工 smoke

- 日期：$(date '+%Y-%m-%d %H:%M:%S %z')
- commit：$(git rev-parse --short HEAD)
- profile：${profile}
- bind：http://127.0.0.1:${port}
- helper：${helper_url}
- 用户名：${username}
- 密码：${password}
- seed：${seed:-none}
- 日志：${log_file}

## 建议检查

- 打开 helper URL
- 自动跳转后 /entries
- 内部导航到 /feeds
- 内部导航到 /settings
- 如使用 \`--seed reader-demo\`，补 \`/entries/2\`
- 刷新当前页仍保持已登录

## 结果补充

- helper：
- /entries：
- /feeds：
- /settings：
- /entries/2：
- 刷新保持登录：
- console：
- 是否通过：
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

bash scripts/run_web_spa_regression_server.sh "${server_args[@]}" >"$log_file" 2>&1 &
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
  echo "Static web browser smoke server failed to become ready on http://127.0.0.1:${port}" >&2
  echo "See log: ${log_file}" >&2
  exit 1
fi

echo "static web browser smoke server is ready on http://127.0.0.1:${port}"
echo "Helper URL: ${helper_url}"
echo "Username: ${username}"
echo "Password: ${password}"
echo "Summary template: ${summary_file}"
echo "Log file: ${log_file}"
echo

wait "$server_pid"
