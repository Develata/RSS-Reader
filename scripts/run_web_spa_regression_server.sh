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

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"

echo "Serving ${public_dir} with SPA fallback on http://127.0.0.1:${port}"
echo "Press Ctrl+C to stop."

python3 - "$public_dir" "$port" "$repo_root" <<'PY'
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
        if not next_path.startswith("/") or next_path.startswith("//"):
            next_path = "/entries"

        core_state = None
        app_state = None
        entry_flags = None

        if seed == "reader-demo":
            fixture_root = os.path.join(repo_root, "tests", "fixtures", "browser_state")
            with open(os.path.join(fixture_root, "reader_demo_core.json"), "r", encoding="utf-8") as fh:
                core_state = json.load(fh)
            with open(os.path.join(fixture_root, "reader_demo_app_state.json"), "r", encoding="utf-8") as fh:
                app_state = json.load(fh)
            with open(os.path.join(fixture_root, "reader_demo_entry_flags.json"), "r", encoding="utf-8") as fh:
                entry_flags = json.load(fh)

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
    const username = {username!r}.trim();
    const password = {password!r};
    const nextPath = {next_path!r};
    const seed = {seed!r};
    const AUTH_CONFIG_KEY = "rssr-web-auth-config-v1";
    const AUTH_SESSION_KEY = "rssr-web-auth-session-v1";
    const STORAGE_KEY = "rssr-web-state-v1";
    const APP_STATE_STORAGE_KEY = "rssr-web-app-state-v1";
    const ENTRY_FLAGS_STORAGE_KEY = "rssr-web-entry-flags-v1";
    const coreState = {json.dumps(core_state, ensure_ascii=False)};
    const appState = {json.dumps(app_state, ensure_ascii=False)};
    const entryFlags = {json.dumps(entry_flags, ensure_ascii=False)};

    function toBase64Url(bytes) {{
      let binary = "";
      for (const value of bytes) binary += String.fromCharCode(value);
      return btoa(binary).replace(/\\+/g, "-").replace(/\\//g, "_").replace(/=+$/g, "");
    }}

    async function sha256Base64Url(text) {{
      const encoded = new TextEncoder().encode(text);
      const digest = await crypto.subtle.digest("SHA-256", encoded);
      return toBase64Url(new Uint8Array(digest));
    }}

    async function main() {{
      const salt = await sha256Base64Url(`${{username}}:codex-static-smoke`);
      const passwordHash = await sha256Base64Url(`${{username}}\\n${{password}}\\n${{salt}}`);
      const sessionToken = await sha256Base64Url(`${{username}}:${{passwordHash}}`);
      localStorage.setItem(AUTH_CONFIG_KEY, `${{username}}\\n${{passwordHash}}\\n${{salt}}`);
      sessionStorage.setItem(AUTH_SESSION_KEY, sessionToken);
      if (seed === "reader-demo" && coreState && appState && entryFlags) {{
        localStorage.setItem(STORAGE_KEY, JSON.stringify(coreState));
        localStorage.setItem(APP_STATE_STORAGE_KEY, JSON.stringify(appState));
        localStorage.setItem(ENTRY_FLAGS_STORAGE_KEY, JSON.stringify(entryFlags));
      }}
      location.replace(nextPath);
    }}

    main().catch((error) => {{
      document.body.innerHTML = `<pre>${{String(error)}}</pre>`;
    }});
  </script>
</body>
</html>"""
        encoded = html.encode("utf-8")
        self.send_response(200)
        self.send_header("Content-Type", "text/html; charset=utf-8")
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
      "rssr-web-app-state-v1",
      "rssr-web-entry-flags-v1",
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
        existing = self._translate_existing_path()
        if existing is not None:
            return super().do_GET()
        self.path = "/index.html"
        return super().do_GET()

    def do_HEAD(self):
        if urlparse(self.path).path in {HELPER_PATH, DUMP_PATH}:
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
