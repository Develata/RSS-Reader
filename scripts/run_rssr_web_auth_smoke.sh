#!/usr/bin/env bash
# Existing deployment-shell assertions extracted from the aggregate entry point.
set -euo pipefail
profile="${1:?missing profile}"
web_port="${2:?missing web port}"
log_dir="${3:?missing log directory}"
public_dir="target/dx/rssr-app/${profile}/web/public"
web_log="$log_dir/rssr-web.log"
trap 'exit 130' INT
trap 'exit 143' TERM
  auth_state_file="$log_dir/rssr-web-auth.json"
  entries_headers="$log_dir/rssr-web-entries.headers"
  login_headers="$log_dir/rssr-web-login.headers"
  login_post_headers="$log_dir/rssr-web-login-post.headers"
  probe_headers="$log_dir/rssr-web-session-probe.headers"
  feeds_headers="$log_dir/rssr-web-feeds.headers"
  settings_headers="$log_dir/rssr-web-settings.headers"
  logout_headers="$log_dir/rssr-web-logout.headers"
  cookie_jar="$log_dir/rssr-web.cookies"
  pid=""

  # Refuse an existing listener rather than accepting its health/login responses.
  python3 - "$web_port" <<'PY'
import socket, sys
with socket.socket() as probe:
    probe.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    try:
        probe.bind(('127.0.0.1', int(sys.argv[1])))
    except OSError as error:
        sys.exit(f'rssr-web smoke port is unavailable: {error}')
PY

  RSS_READER_WEB_BIND="127.0.0.1:${web_port}" \
  RSS_READER_WEB_STATIC_DIR="$public_dir" \
  RSS_READER_WEB_USERNAME="smoke" \
  RSS_READER_WEB_PASSWORD="smoke-pass-123" \
  RSS_READER_WEB_SESSION_SECRET="release-ui-regression-session-secret-0123456789" \
  RSS_READER_WEB_AUTH_STATE_FILE="$auth_state_file" \
  cargo run --locked -p rssr-web >"$web_log" 2>&1 &
  pid=$!

  trap '
    if [[ -n "$pid" ]] && kill -0 "$pid" >/dev/null 2>&1; then
      kill "$pid" >/dev/null 2>&1 || true
      wait "$pid" >/dev/null 2>&1 || true
    fi
  ' EXIT

  curl() { command curl --connect-timeout 2 --max-time 10 "$@"; }

  ready="false"
  for _ in {1..30}; do
    if ! kill -0 "$pid" 2>/dev/null; then
      echo "rssr-web exited before readiness; see $web_log" >&2
      exit 1
    fi
    if curl -fsS "http://127.0.0.1:${web_port}/healthz" >/dev/null 2>&1; then
      ready="true"
      break
    fi
    sleep 1
  done
  if [[ "$ready" != true ]]; then
    echo "rssr-web did not become ready; see $web_log" >&2
    exit 1
  fi

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
  kill -0 "$pid"
