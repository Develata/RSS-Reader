#!/usr/bin/env bash
# Single source for deployment HTTP assertions; release-ui owns the service.
set -euo pipefail
web_port="${1:?missing web port}"
log_dir="${2:?missing log directory}"
trap 'exit 130' INT
trap 'exit 143' TERM
entries_headers="$log_dir/rssr-web-entries.headers"
login_headers="$log_dir/rssr-web-login.headers"
login_post_headers="$log_dir/rssr-web-login-post.headers"
probe_headers="$log_dir/rssr-web-session-probe.headers"
feeds_headers="$log_dir/rssr-web-feeds.headers"
settings_headers="$log_dir/rssr-web-settings.headers"
logout_headers="$log_dir/rssr-web-logout.headers"
cookie_jar="$log_dir/rssr-web.cookies"

curl() { command curl --connect-timeout 2 --max-time 10 "$@"; }

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
