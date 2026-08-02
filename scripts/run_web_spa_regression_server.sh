#!/usr/bin/env bash
set -euo pipefail

profile="debug"
port="8091"
skip_build="false"

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
    *)
      echo "Usage: $0 [--port PORT] [--debug|--release] [--skip-build]" >&2
      exit 1
      ;;
  esac
done

if [[ "$skip_build" != "true" ]]; then
  echo "Building rssr-app web bundle (${profile})..."
  if [[ "$profile" == "release" ]]; then
    dx build --platform web --package rssr-app --release >/dev/null
  else
    dx build --platform web --package rssr-app >/dev/null
  fi
fi

public_dir="target/dx/rssr-app/${profile}/web/public"
if [[ ! -d "$public_dir" ]]; then
  echo "Web build output not found: $public_dir" >&2
  exit 1
fi

if [[ -n "${RSSR_REPO_ROOT:-}" ]]; then
  repo_root="${RSSR_REPO_ROOT}"
else
  script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  repo_root="$(cd "$script_dir/.." && pwd)"
fi

echo "Serving ${public_dir} with SPA fallback on http://127.0.0.1:${port}"
echo "Press Ctrl+C to stop."

python3 - "$public_dir" "$port" "$repo_root" <<'PY'
import base64
import hashlib
import http.server
import json
import os
import socketserver
import sys
from functools import partial
from urllib.parse import parse_qs, urlparse

root = os.path.abspath(sys.argv[1])
port = int(sys.argv[2])
repo_root = os.path.abspath(sys.argv[3])
HELPER_PATH = "/__codex/setup-local-auth"
DUMP_PATH = "/__codex/dump-browser-state"
MOBILE_UI_FEED_PATH = "/__codex/mobile-ui-feed.xml"
THEME_FIXTURE_ROOT = os.path.join(repo_root, "assets", "themes")
THEME_PRESET_FILES = {
    "atlas-sidebar": "atlas-sidebar.css",
    "newsprint": "newsprint.css",
    "amethyst-glass": "amethyst-glass.css",
    "midnight-ledger": "midnight-ledger.css",
}
BROWSER_STATE_SEEDS = {
    "reader-demo": "reader_demo",
    "mobile-ui-overflow": "mobile_ui_overflow",
    "mobile-ui-short": "mobile_ui_short",
}


def load_theme_preset_css(key):
    filename = THEME_PRESET_FILES.get(key)
    if not filename:
        return None
    with open(os.path.join(THEME_FIXTURE_ROOT, filename), "r", encoding="utf-8") as fh:
        return fh.read()


def load_browser_state_seed(key):
    prefix = BROWSER_STATE_SEEDS.get(key)
    if prefix is None:
        return None

    fixture_root = os.path.join(repo_root, "tests", "fixtures", "browser_state")
    result = []
    for suffix in ("core", "app_state", "entry_flags", "entry_content"):
        path = os.path.join(fixture_root, f"{prefix}_{suffix}.json")
        with open(path, "r", encoding="utf-8") as fh:
            result.append(json.load(fh))
    return tuple(result)


def to_base64_url(raw_bytes):
    return base64.urlsafe_b64encode(raw_bytes).decode("ascii").rstrip("=")


def sha256_base64_url(text):
    return to_base64_url(hashlib.sha256(text.encode("utf-8")).digest())


class SpaFallbackHandler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *args, directory=None, **kwargs):
        super().__init__(*args, directory=directory, **kwargs)

    def _translate_existing_path(self):
        path = self.translate_path(self.path)
        if os.path.isdir(path):
            index = os.path.join(path, "index.html")
            if os.path.exists(index):
                return index
        if os.path.exists(path):
            return path
        return None

    def _auth_helper_page(self):
        parsed = urlparse(self.path)
        params = parse_qs(parsed.query, keep_blank_values=False)
        username = params.get("username", ["smoke"])[0].strip()
        password = params.get("password", ["smoke-pass-123"])[0]
        next_path = params.get("next", ["/entries"])[0]
        seed = params.get("seed", [""])[0].strip()
        preset = params.get("preset", [""])[0].strip()
        if not next_path.startswith("/") or next_path.startswith("//"):
            next_path = "/entries"

        salt = sha256_base64_url(f"{username}:codex-static-smoke")
        password_hash = sha256_base64_url(f"{username}\n{password}\n{salt}")
        session_token = sha256_base64_url(f"{username}:{password_hash}")

        core_state = None
        app_state = None
        entry_flags = None
        entry_content = None

        if seed:
            loaded_seed = load_browser_state_seed(seed)
            if loaded_seed is None:
                self.send_error(400, f"Unknown browser state seed: {seed}")
                return
            core_state, app_state, entry_flags, entry_content = loaded_seed
            if seed.startswith("mobile-ui-"):
                feed_url = f"http://127.0.0.1:{port}{MOBILE_UI_FEED_PATH}?seed={seed}"
                for feed in core_state.get("feeds", []):
                    feed["url"] = feed_url

        preset_css = load_theme_preset_css(preset)
        if core_state is not None and preset_css is not None:
            core_state.setdefault("settings", {})["custom_css"] = preset_css

        html = f"""<!doctype html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Static Web Auth Helper</title>
</head>
<body>
  <p>Preparing local web auth for <code>{username}</code>...</p>
  <script>
    const nextPath = {next_path!r};
    const preset = {preset!r};
    const AUTH_CONFIG_KEY = "rssr-web-auth-config-v1";
    const AUTH_SESSION_KEY = "rssr-web-auth-session-v1";
    const STORAGE_KEY = "rssr-web-state-v1";
    const APP_STATE_STORAGE_KEY = "rssr-web-app-state-v2";
    const ENTRY_FLAGS_STORAGE_KEY = "rssr-web-entry-flags-v1";
    const ENTRY_CONTENT_STORAGE_KEY = "rssr-web-entry-content-v1";
    const authConfig = {f"{username}\n{password_hash}\n{salt}"!r};
    const sessionToken = {session_token!r};
    const coreState = {json.dumps(core_state, ensure_ascii=False)};
    const appState = {json.dumps(app_state, ensure_ascii=False)};
    const entryFlags = {json.dumps(entry_flags, ensure_ascii=False)};
    const entryContent = {json.dumps(entry_content, ensure_ascii=False)};

    function main() {{
      localStorage.setItem(AUTH_CONFIG_KEY, authConfig);
      sessionStorage.setItem(AUTH_SESSION_KEY, sessionToken);
      if (coreState && appState && entryFlags && entryContent) {{
        localStorage.setItem(STORAGE_KEY, JSON.stringify(coreState));
        localStorage.setItem(APP_STATE_STORAGE_KEY, JSON.stringify(appState));
        localStorage.setItem(ENTRY_FLAGS_STORAGE_KEY, JSON.stringify(entryFlags));
        localStorage.setItem(ENTRY_CONTENT_STORAGE_KEY, JSON.stringify(entryContent));
      }}
      location.replace(nextPath);
    }}

    try {{
      main();
    }} catch (error) {{
      document.body.innerHTML = `<pre>${{String(error)}}</pre>`;
    }}
  </script>
</body>
</html>"""
        encoded = html.encode("utf-8")
        self.send_response(200)
        self.send_header("Content-Type", "text/html; charset=utf-8")
        self.send_header("Content-Length", str(len(encoded)))
        self.end_headers()
        self.wfile.write(encoded)

    def _mobile_ui_feed(self):
        seed = parse_qs(urlparse(self.path).query).get("seed", ["mobile-ui-overflow"])[0]
        if seed == "mobile-ui-short":
            title = "Short Mobile Fixture"
            guid = "mobile-ui-short-entry"
            entry_title = "Short directory fixture entry"
        else:
            title = "用于验证移动端来源筛选单行截断与完整可访问名称的超长订阅源标题 RSS Reader Mobile Regression Fixture"
            guid = "mobile-ui-2026-08"
            entry_title = "2026 年 8 月移动端目录回归条目"

        xml = f"""<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>{title}</title>
    <link>https://example.com/mobile-ui</link>
    <description>Deterministic mobile UI smoke feed.</description>
    <item>
      <guid>{guid}</guid>
      <link>https://example.com/mobile-ui/{guid}</link>
      <title>{entry_title}</title>
      <pubDate>Sat, 01 Aug 2026 08:00:00 GMT</pubDate>
      <description>Deterministic mobile UI smoke entry.</description>
    </item>
  </channel>
</rss>"""
        encoded = xml.encode("utf-8")
        self.send_response(200)
        self.send_header("Content-Type", "application/rss+xml; charset=utf-8")
        self.send_header("Cache-Control", "no-store")
        self.send_header("Content-Length", str(len(encoded)))
        self.end_headers()
        self.wfile.write(encoded)

    def _dump_browser_state_page(self):
        html = """<!doctype html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Browser State Dump</title>
</head>
<body>
  <pre id="dump">loading...</pre>
  <script>
    const keys = [
      "rssr-web-auth-config-v1",
      "rssr-web-auth-session-v1",
      "rssr-web-state-v1",
      "rssr-web-app-state-v2",
      "rssr-web-entry-flags-v1",
      "rssr-web-entry-content-v1",
    ];

    function safeParse(raw) {
      if (raw == null) return null;
      try {
        return JSON.parse(raw);
      } catch (error) {
        return { parse_error: String(error), raw };
      }
    }

    const result = {
      auth_config_present: localStorage.getItem(keys[0]) != null,
      auth_session_present: sessionStorage.getItem(keys[1]) != null,
      core: safeParse(localStorage.getItem(keys[2])),
      app_state: safeParse(localStorage.getItem(keys[3])),
      entry_flags: safeParse(localStorage.getItem(keys[4])),
      entry_content: safeParse(localStorage.getItem(keys[5])),
    };

    document.getElementById("dump").textContent = JSON.stringify(result, null, 2);
  </script>
</body>
</html>"""
        encoded = html.encode("utf-8")
        self.send_response(200)
        self.send_header("Content-Type", "text/html; charset=utf-8")
        self.send_header("Content-Length", str(len(encoded)))
        self.end_headers()
        self.wfile.write(encoded)

    def do_GET(self):
        path = urlparse(self.path).path
        if path == HELPER_PATH:
            return self._auth_helper_page()
        if path == DUMP_PATH:
            return self._dump_browser_state_page()
        if path == MOBILE_UI_FEED_PATH:
            return self._mobile_ui_feed()
        existing = self._translate_existing_path()
        if existing is not None:
            return super().do_GET()
        self.path = "/index.html"
        return super().do_GET()

    def do_HEAD(self):
        if urlparse(self.path).path in {HELPER_PATH, DUMP_PATH, MOBILE_UI_FEED_PATH}:
            self.send_response(200)
            self.send_header("Content-Type", "text/html; charset=utf-8")
            self.end_headers()
            return
        existing = self._translate_existing_path()
        if existing is not None:
            return super().do_HEAD()
        self.path = "/index.html"
        return super().do_HEAD()


handler = partial(SpaFallbackHandler, directory=root)
with socketserver.TCPServer(("127.0.0.1", port), handler) as httpd:
    httpd.serve_forever()
PY
