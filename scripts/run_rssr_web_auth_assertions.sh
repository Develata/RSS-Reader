#!/usr/bin/env bash
# Single source for deployment HTTP assertions; release-ui owns the service.
set -euo pipefail
web_port="${1:?missing web port}"
log_dir="${2:?missing log directory}"
trap 'exit 130' INT
trap 'exit 143' TERM
entries_headers="rssr-web-entries.headers"
login_headers="rssr-web-login.headers"
login_post_headers="rssr-web-login-post.headers"
probe_headers="rssr-web-session-probe.headers"
feeds_headers="rssr-web-feeds.headers"
settings_headers="rssr-web-settings.headers"
logout_headers="rssr-web-logout.headers"
cookie_jar="rssr-web.cookies"

# Resolve against the caller's PATH before changing cwd, including relative
# entries. Native Windows curl builds may only support ASCII filename arguments;
# the OS cwd remains Unicode and all output stays in the requested log directory.
curl_bin="$(type -P curl)"
case "$curl_bin" in
  /* | [[:alpha:]]:[/\\]*) ;;
  *) curl_bin="$PWD/$curl_bin" ;;
esac
curl() (
  cd -- "$log_dir"
  "$curl_bin" --connect-timeout 2 --max-time 10 "$@"
)

curl -fsS -D "$login_headers" -o /dev/null "http://127.0.0.1:${web_port}/login"
curl -sS -D "$entries_headers" -o /dev/null "http://127.0.0.1:${web_port}/entries"

grep -q "200 OK" "$log_dir/$login_headers"
grep -Eq "^HTTP/.* 30[237]" "$log_dir/$entries_headers"
grep -Eq "location: /login|Location: /login" "$log_dir/$entries_headers"

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

grep -Eq "^HTTP/.* 30[237]" "$log_dir/$login_post_headers"
grep -Eq "location: /feeds|Location: /feeds" "$log_dir/$login_post_headers"

curl -sS -b "$cookie_jar" -D "$probe_headers" -o /dev/null "http://127.0.0.1:${web_port}/session-probe"
curl -sS -b "$cookie_jar" -D "$feeds_headers" -o /dev/null "http://127.0.0.1:${web_port}/feeds"
curl -sS -b "$cookie_jar" -D "$settings_headers" -o /dev/null "http://127.0.0.1:${web_port}/settings"

grep -Eq "^HTTP/.* 204" "$log_dir/$probe_headers"
grep -Eq "^HTTP/.* 200" "$log_dir/$feeds_headers"
grep -Eq "^HTTP/.* 200" "$log_dir/$settings_headers"

curl -sS -b "$cookie_jar" -D "$logout_headers" -o /dev/null "http://127.0.0.1:${web_port}/logout"
grep -Eq "^HTTP/.* 30[237]" "$log_dir/$logout_headers"
grep -Eq "location: /login|Location: /login" "$log_dir/$logout_headers"
