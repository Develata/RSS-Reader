#!/usr/bin/env bash
# Single source for deployment HTTP assertions; release-ui owns the service.
set -euo pipefail
web_port="${1:?missing web port}"
log_dir="${2:?missing log directory}"
trap 'exit 130' INT
trap 'exit 143' TERM
origin="http://127.0.0.1:${web_port}"
entries_headers="rssr-web-entries.headers"
login_headers="rssr-web-login.headers"
login_foreign_headers="rssr-web-login-foreign.headers"
login_post_headers="rssr-web-login-post.headers"
probe_headers="rssr-web-session-probe.headers"
feeds_headers="rssr-web-feeds.headers"
settings_headers="rssr-web-settings.headers"
api_session_headers="rssr-web-api-session.headers"
replay_headers="rssr-web-session-replay.headers"
logout_page_headers="rssr-web-logout-page.headers"
logout_headers="rssr-web-logout.headers"
revoked_headers="rssr-web-revoked.headers"
api_unauthenticated_headers="rssr-web-api-unauthenticated.headers"
api_unauthenticated_body="rssr-web-api-unauthenticated.json"
relogin_headers="rssr-web-relogin.headers"
relogin_probe_headers="rssr-web-relogin-probe.headers"
relogin_jar="rssr-web-relogin.cookies"
api_session_body="rssr-web-api-session.json"
logout_page="rssr-web-logout.html"
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

curl -fsS -D "$login_headers" -o /dev/null "${origin}/login"
curl -sS -D "$entries_headers" -o /dev/null "${origin}/entries"

grep -q "200 OK" "$log_dir/$login_headers"
grep -Eq "^HTTP/.* 30[237]" "$log_dir/$entries_headers"
grep -Eq "location: /login|Location: /login" "$log_dir/$entries_headers"

# A write from another origin (here the same host on another port) is refused.
curl -sS \
  -D "$login_foreign_headers" \
  -o /dev/null \
  -X POST \
  -H "Origin: http://127.0.0.1:1" \
  --data-urlencode "username=smoke" \
  --data-urlencode "password=smoke-pass-123" \
  --data-urlencode "next=/feeds" \
  "${origin}/login"

grep -Eq "^HTTP/.* 403" "$log_dir/$login_foreign_headers"

curl -sS \
  -c "$cookie_jar" \
  -b "$cookie_jar" \
  -D "$login_post_headers" \
  -o /dev/null \
  -X POST \
  -H "Origin: ${origin}" \
  --data-urlencode "username=smoke" \
  --data-urlencode "password=smoke-pass-123" \
  --data-urlencode "next=/feeds" \
  "${origin}/login"

grep -Eq "^HTTP/.* 30[237]" "$log_dir/$login_post_headers"
grep -Eq "location: /feeds|Location: /feeds" "$log_dir/$login_post_headers"
# Native Windows curl writes the jar with CRLF line endings.
session_cookie="$(awk -F '\t' '$6 == "rssr_web_session" { sub(/\r$/, "", $7); print $7 }' "$log_dir/$cookie_jar")"
[[ -n "$session_cookie" ]]

curl -sS -b "$cookie_jar" -D "$probe_headers" -o /dev/null "${origin}/session-probe"
curl -sS -b "$cookie_jar" -D "$feeds_headers" -o /dev/null "${origin}/feeds"
curl -sS -b "$cookie_jar" -D "$settings_headers" -o /dev/null "${origin}/settings"
curl -sS -b "$cookie_jar" -D "$api_session_headers" -o "$api_session_body" "${origin}/api/v1/session"

grep -Eq "^HTTP/.* 204" "$log_dir/$probe_headers"
grep -Eq "^HTTP/.* 200" "$log_dir/$feeds_headers"
grep -Eq "^HTTP/.* 200" "$log_dir/$settings_headers"
grep -Eq "^HTTP/.* 200" "$log_dir/$api_session_headers"
grep -Eiq "^content-type: application/json" "$log_dir/$api_session_headers"
grep -Eiq "^cache-control: no-store" "$log_dir/$api_session_headers"
grep -q '"csrf_token"' "$log_dir/$api_session_body"

# Control for the replay below: the extracted session cookie works on its own.
curl -sS -H "Cookie: rssr_web_session=${session_cookie}" -D "$replay_headers" -o /dev/null \
  "${origin}/session-probe"
grep -Eq "^HTTP/.* 204" "$log_dir/$replay_headers"

# Logging out: GET shows the confirmation page with the session's CSRF token, POST revokes.
curl -sS -b "$cookie_jar" -D "$logout_page_headers" -o "$logout_page" "${origin}/logout"
grep -Eq "^HTTP/.* 200" "$log_dir/$logout_page_headers"
csrf="$(sed -n 's/.*name="csrf" value="\([^"]*\)".*/\1/p' "$log_dir/$logout_page")"
[[ -n "$csrf" ]]

# The jar keeps what the login set; the logout response must clear both cookies.
curl -sS \
  -b "$cookie_jar" \
  -D "$logout_headers" \
  -o /dev/null \
  -X POST \
  -H "Origin: ${origin}" \
  --data-urlencode "csrf=${csrf}" \
  "${origin}/logout"
grep -Eq "^HTTP/.* 30[237]" "$log_dir/$logout_headers"
grep -Eq "location: /login|Location: /login" "$log_dir/$logout_headers"
grep -Eiq "^set-cookie: rssr_web_session=deleted;.*Max-Age=0" "$log_dir/$logout_headers"
grep -Eiq "^set-cookie: rssr_web_gate=deleted;.*Max-Age=0" "$log_dir/$logout_headers"

# The server revoked the session: replaying the old cookie is refused.
curl -sS -H "Cookie: rssr_web_session=${session_cookie}" -D "$revoked_headers" -o /dev/null \
  "${origin}/session-probe"
grep -Eq "^HTTP/.* 30[237]" "$log_dir/$revoked_headers"
grep -Eq "(location|Location): /login\?error=session_expired" "$log_dir/$revoked_headers"

# Without a session the API answers JSON 401, never the app.
curl -sS -D "$api_unauthenticated_headers" -o "$api_unauthenticated_body" "${origin}/api/v1/session"
grep -Eq "^HTTP/.* 401" "$log_dir/$api_unauthenticated_headers"
grep -Eiq "^content-type: application/json" "$log_dir/$api_unauthenticated_headers"
grep -q '"unauthenticated"' "$log_dir/$api_unauthenticated_body"

# Logging in again gives a new session, independent of the revoked one.
curl -sS \
  -c "$relogin_jar" \
  -D "$relogin_headers" \
  -o /dev/null \
  -X POST \
  -H "Origin: ${origin}" \
  --data-urlencode "username=smoke" \
  --data-urlencode "password=smoke-pass-123" \
  --data-urlencode "next=/feeds" \
  "${origin}/login"
grep -Eq "^HTTP/.* 30[237]" "$log_dir/$relogin_headers"
new_session_cookie="$(awk -F '\t' '$6 == "rssr_web_session" { sub(/\r$/, "", $7); print $7 }' "$log_dir/$relogin_jar")"
[[ -n "$new_session_cookie" && "$new_session_cookie" != "$session_cookie" ]]
curl -sS -H "Cookie: rssr_web_session=${new_session_cookie}" -D "$relogin_probe_headers" \
  -o /dev/null "${origin}/session-probe"
grep -Eq "^HTTP/.* 204" "$log_dir/$relogin_probe_headers"
