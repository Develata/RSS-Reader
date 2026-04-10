#!/usr/bin/env bash
set -euo pipefail

profile="debug"
port="18081"
skip_build="false"
log_dir=""
username="smoke"
password="smoke-pass-123"
session_secret="rssr-web-proxy-feed-smoke-session-secret-0123456789"
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
  log_dir="target/rssr-web-proxy-feed-smoke/$(date +%Y%m%d-%H%M%S)"
fi

mkdir -p "$log_dir"

public_dir="target/dx/rssr-app/${profile}/web/public"
auth_state_file="$log_dir/rssr-web-auth.json"
server_log="$log_dir/rssr-web.log"
summary_file="$log_dir/summary.md"
cookie_jar="$log_dir/rssr-web.cookies"
login_headers="$log_dir/login.headers"
proxy_headers="$log_dir/feed-proxy.headers"
proxy_body="$log_dir/feed-proxy.xml"

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
# rssr-web 代理 feed Smoke

- 日期：$(date '+%Y-%m-%d %H:%M:%S %z')
- commit：$(git rev-parse --short HEAD)
- profile：${profile}
- bind：http://127.0.0.1:${port}
- feed：${feed_url}
- 日志：${server_log}

## 结果

- /login：
- 登录：
- /feed-proxy：
- content-type：
- XML body：
- 是否通过：
EOF

RSS_READER_WEB_BIND="127.0.0.1:${port}" \
RSS_READER_WEB_STATIC_DIR="$public_dir" \
RSS_READER_WEB_USERNAME="$username" \
RSS_READER_WEB_PASSWORD="$password" \
RSS_READER_WEB_SESSION_SECRET="$session_secret" \
RSS_READER_WEB_AUTH_STATE_FILE="$auth_state_file" \
cargo run -p rssr-web >"$server_log" 2>&1 &
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
  echo "rssr-web proxy feed smoke server failed to become ready on http://127.0.0.1:${port}" >&2
  echo "See log: ${server_log}" >&2
  exit 1
fi

curl -sS \
  -c "$cookie_jar" \
  -b "$cookie_jar" \
  -D "$login_headers" \
  -o /dev/null \
  -X POST \
  --data-urlencode "username=${username}" \
  --data-urlencode "password=${password}" \
  --data-urlencode "next=/feeds" \
  "http://127.0.0.1:${port}/login"

grep -Eq "^HTTP/.* 30[237]" "$login_headers"
grep -Eq "location: /feeds|Location: /feeds" "$login_headers"

curl -sS \
  -b "$cookie_jar" \
  -D "$proxy_headers" \
  --get \
  --data-urlencode "url=${feed_url}" \
  -o "$proxy_body" \
  "http://127.0.0.1:${port}/feed-proxy"

grep -Eq "^HTTP/.* 200" "$proxy_headers"
if grep -Eiq "^content-type: text/html|^content-type: application/xhtml\\+xml" "$proxy_headers"; then
  echo "feed proxy returned HTML instead of XML" >&2
  exit 1
fi

if ! rg -q "<feed\\b|<rss\\b|<rdf:RDF\\b" "$proxy_body"; then
  echo "feed proxy body did not look like XML feed" >&2
  exit 1
fi

if rg -qi "<!doctype html|<html\\b|/login" "$proxy_body"; then
  echo "feed proxy body looked like login page or HTML shell" >&2
  exit 1
fi

echo "rssr-web proxy feed smoke passed"
echo "Summary template: ${summary_file}"
echo "Artifacts: ${log_dir}"
