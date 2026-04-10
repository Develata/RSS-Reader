#!/usr/bin/env bash
set -euo pipefail

profile="debug"
port="8091"
web_port="18081"
skip_automated="false"
skip_build="false"
serve_spa="true"
with_rssr_web="false"
log_dir=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --port)
      port="${2:?missing port value}"
      shift 2
      ;;
    --web-port)
      web_port="${2:?missing web port value}"
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
    --skip-automated)
      skip_automated="true"
      shift
      ;;
    --skip-build)
      skip_build="true"
      shift
      ;;
    --with-rssr-web)
      with_rssr_web="true"
      shift
      ;;
    --log-dir)
      log_dir="${2:?missing log dir value}"
      shift 2
      ;;
    --no-serve)
      serve_spa="false"
      shift
      ;;
    *)
      echo "Usage: $0 [--port PORT] [--web-port PORT] [--debug|--release] [--skip-automated] [--skip-build] [--with-rssr-web] [--log-dir DIR] [--no-serve]" >&2
      exit 1
      ;;
  esac
done

if [[ -z "$log_dir" ]]; then
  log_dir="target/release-ui-regression/$(date +%Y%m%d-%H%M%S)"
fi

mkdir -p "$log_dir"

summary_file="$log_dir/summary.md"
automated_log="$log_dir/automated-gates.log"
web_log="$log_dir/rssr-web.log"
web_browser_feed_log="$log_dir/rssr-web-browser-feed-smoke.log"

write_summary() {
  local automated_status="$1"
  local web_status="$2"
  local spa_status="$3"

  cat >"$summary_file" <<EOF
# 发布前 UI 预检结果

- 日期：$(date '+%Y-%m-%d %H:%M:%S %z')
- commit：$(git rev-parse --short HEAD)
- profile：${profile}
- 静态 Web 端口：${port}
- rssr-web 端口：${web_port}
- 日志目录：${log_dir}

## 状态

- 自动化门禁：${automated_status}
- rssr-web smoke：${web_status}
- 静态 Web + SPA fallback：${spa_status}

## 结果记录补充

- 执行环境：
- env-limited 项：
- /entries：
- /feeds：
- /settings：
- /reader/{entry_id}：
- 静态 reader seed smoke：
- 默认主题：
- Atlas Sidebar：
- Newsprint：
- Amethyst Glass：
- Midnight Ledger：
- 是否允许发布：
EOF
}

public_dir="target/dx/rssr-app/${profile}/web/public"

ensure_web_bundle() {
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
}

run_rssr_web_smoke() {
  local auth_state_file="$log_dir/rssr-web-auth.json"
  local entries_headers="$log_dir/rssr-web-entries.headers"
  local login_headers="$log_dir/rssr-web-login.headers"
  local login_post_headers="$log_dir/rssr-web-login-post.headers"
  local probe_headers="$log_dir/rssr-web-session-probe.headers"
  local feeds_headers="$log_dir/rssr-web-feeds.headers"
  local settings_headers="$log_dir/rssr-web-settings.headers"
  local logout_headers="$log_dir/rssr-web-logout.headers"
  local cookie_jar="$log_dir/rssr-web.cookies"
  local pid=""

  RSS_READER_WEB_BIND="127.0.0.1:${web_port}" \
  RSS_READER_WEB_STATIC_DIR="$public_dir" \
  RSS_READER_WEB_USERNAME="smoke" \
  RSS_READER_WEB_PASSWORD="smoke-pass-123" \
  RSS_READER_WEB_SESSION_SECRET="release-ui-regression-session-secret-0123456789" \
  RSS_READER_WEB_AUTH_STATE_FILE="$auth_state_file" \
  cargo run -p rssr-web >"$web_log" 2>&1 &
  pid=$!

  cleanup() {
    if [[ -n "$pid" ]] && kill -0 "$pid" >/dev/null 2>&1; then
      kill "$pid" >/dev/null 2>&1 || true
      wait "$pid" >/dev/null 2>&1 || true
    fi
  }
  trap cleanup RETURN

  for _ in {1..30}; do
    if curl -fsS "http://127.0.0.1:${web_port}/healthz" >/dev/null 2>&1; then
      break
    fi
    sleep 1
  done

  curl -fsS -D "$login_headers" -o /dev/null "http://127.0.0.1:${web_port}/login"
  curl -sS -D "$entries_headers" -o /dev/null "http://127.0.0.1:${web_port}/entries"

  grep -q "200 OK" "$login_headers"
  grep -Eq "^HTTP/.* 30[237]" "$entries_headers"
  grep -Eq "location: /login|Location: /login" "$entries_headers"

  curl -sS \
    -c "$cookie_jar" \
    -b "$cookie_jar" \
    -D "$login_post_headers" \
    -o /dev/null \
    -X POST \
    --data-urlencode "username=smoke" \
    --data-urlencode "password=smoke-pass-123" \
    --data-urlencode "next=/feeds" \
    "http://127.0.0.1:${web_port}/login"

  grep -Eq "^HTTP/.* 30[237]" "$login_post_headers"
  grep -Eq "location: /feeds|Location: /feeds" "$login_post_headers"

  curl -sS -b "$cookie_jar" -D "$probe_headers" -o /dev/null "http://127.0.0.1:${web_port}/session-probe"
  curl -sS -b "$cookie_jar" -D "$feeds_headers" -o /dev/null "http://127.0.0.1:${web_port}/feeds"
  curl -sS -b "$cookie_jar" -D "$settings_headers" -o /dev/null "http://127.0.0.1:${web_port}/settings"

  grep -Eq "^HTTP/.* 204" "$probe_headers"
  grep -Eq "^HTTP/.* 200" "$feeds_headers"
  grep -Eq "^HTTP/.* 200" "$settings_headers"

  curl -sS -b "$cookie_jar" -D "$logout_headers" -o /dev/null "http://127.0.0.1:${web_port}/logout"
  grep -Eq "^HTTP/.* 30[237]" "$logout_headers"
  grep -Eq "location: /login|Location: /login" "$logout_headers"
}

write_summary "pending" "skipped" "$(if [[ "$serve_spa" == "true" ]]; then echo pending; else echo skipped; fi)"

if [[ "$skip_automated" != "true" ]]; then
  echo "Running release UI automated gates..."
  {
    cargo check -p rssr-app
    cargo check -p rssr-app --target wasm32-unknown-unknown
    cargo test -p rssr-app
    cargo test -p rssr-app --test test_builtin_theme_contracts
    cargo test -p rssr-infra --test test_subscription_contract_harness
    cargo test -p rssr-infra --test test_config_exchange_contract_harness
    cargo test -p rssr-web
  } 2>&1 | tee "$automated_log"
fi

write_summary "passed" "$(if [[ "$with_rssr_web" == "true" ]]; then echo pending; else echo skipped; fi)" "$(if [[ "$serve_spa" == "true" ]]; then echo pending; else echo skipped; fi)"

if [[ "$with_rssr_web" == "true" ]]; then
  ensure_web_bundle
  echo "Running rssr-web smoke..."
  run_rssr_web_smoke
  echo "Running rssr-web browser feed smoke..."
  bash scripts/run_rssr_web_browser_feed_smoke.sh \
    --skip-build \
    --port "$((web_port + 1))" \
    --log-dir "$log_dir/rssr-web-browser-feed-smoke" \
    >"$web_browser_feed_log" 2>&1
  write_summary "passed" "passed" "$(if [[ "$serve_spa" == "true" ]]; then echo pending; else echo skipped; fi)"
fi

if [[ "$serve_spa" != "true" ]]; then
  echo "Release UI automated gates completed."
  echo "Summary written to $summary_file"
  exit 0
fi

ensure_web_bundle

echo
echo "Automated gates passed. Starting static web regression server..."
echo "After the server comes up, manually verify:"
echo "  - http://127.0.0.1:${port}/entries"
echo "  - http://127.0.0.1:${port}/feeds"
echo "  - http://127.0.0.1:${port}/settings"
echo "  - http://127.0.0.1:${port}/__codex/setup-local-auth?username=smoke&password=smoke-pass-123&seed=reader-demo&next=/entries/2"
if [[ "$with_rssr_web" == "true" ]]; then
  echo "  - rssr-web smoke logs: $web_log"
fi
echo "Summary template: $summary_file"
echo

server_args=(--port "$port")
if [[ "$profile" == "release" ]]; then
  server_args+=(--release)
else
  server_args+=(--debug)
fi
if [[ "$skip_build" == "true" ]]; then
  server_args+=(--skip-build)
fi

exec bash scripts/run_web_spa_regression_server.sh "${server_args[@]}"
