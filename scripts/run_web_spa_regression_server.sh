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

echo "Serving ${public_dir} with SPA fallback on http://127.0.0.1:${port}"
echo "Press Ctrl+C to stop."

python3 - "$public_dir" "$port" <<'PY'
import http.server
import os
import socketserver
import sys
from functools import partial

root = os.path.abspath(sys.argv[1])
port = int(sys.argv[2])


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

    def do_GET(self):
        existing = self._translate_existing_path()
        if existing is not None:
            return super().do_GET()
        self.path = "/index.html"
        return super().do_GET()

    def do_HEAD(self):
        existing = self._translate_existing_path()
        if existing is not None:
            return super().do_HEAD()
        self.path = "/index.html"
        return super().do_HEAD()


handler = partial(SpaFallbackHandler, directory=root)
with socketserver.TCPServer(("127.0.0.1", port), handler) as httpd:
    httpd.serve_forever()
PY
