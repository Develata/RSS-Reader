"""Local HTTP peer for web-auth lifecycle and unchanged shell assertion contracts."""
import http.server
import json
import os
from pathlib import Path
import subprocess
import sys
import threading
import time
import urllib.parse

if "--leaf" in sys.argv:
    time.sleep(300)
    raise SystemExit()

lock = threading.Lock()
health_count = 0
leaf = subprocess.Popen([sys.executable, __file__, "--leaf"])
Path("auth-pids.json").write_text(json.dumps([os.getpid(), leaf.pid]), encoding="utf-8")


class Handler(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def reply(self, status, headers=None):
        self.send_response(status)
        for key, value in (headers or {}).items():
            self.send_header(key, value)
        self.send_header("Content-Length", "0")
        self.end_headers()

    def do_GET(self):
        global health_count
        with lock:
            with Path("auth-requests.jsonl").open("a", encoding="utf-8") as out:
                out.write(json.dumps({"method": "GET", "path": self.path,
                                      "cookie": self.headers.get("Cookie", "")}) + "\n")
            if self.path == "/healthz":
                health_count += 1
                count = health_count
        if self.path == "/healthz":
            if count <= int(os.environ.get("AUTH_SLOW_ATTEMPTS", "0")):
                time.sleep(float(os.environ.get("AUTH_DELAY", "11")))
            self.reply(200)
        elif self.path == "/login":
            self.reply(200)
        elif self.path == "/entries":
            self.reply(302, {"Location": "/login"})
        elif self.path in ["/session-probe", "/feeds", "/settings", "/logout"]:
            if self.headers.get("Cookie") != "rssr-session=fixture-token":
                self.reply(401)
                return
            if os.environ.get("AUTH_EXIT_DURING_ASSERTIONS") and self.path == "/feeds":
                os._exit(17)
            if self.path == "/logout":
                self.reply(303, {"Location": "/login"})
            else:
                self.reply(204 if self.path == "/session-probe" else 200)
        else:
            self.reply(404)

    def do_POST(self):
        body = self.rfile.read(int(self.headers["Content-Length"])).decode("ascii")
        form = urllib.parse.parse_qs(body)
        with lock:
            with Path("auth-requests.jsonl").open("a", encoding="utf-8") as out:
                out.write(json.dumps({"method": "POST", "path": self.path, "form": form}) + "\n")
        if self.path != "/login" or form != {
            "username": ["smoke"], "password": ["smoke-pass-123"], "next": ["/feeds"]}:
            self.reply(422)
            return
        self.reply(303, {
            "Location": "/wrong" if os.environ.get("AUTH_BAD_REDIRECT") else "/feeds",
            "Set-Cookie": "rssr-session=fixture-token; Path=/; HttpOnly; SameSite=Lax",
        })


host, port = os.environ["RSS_READER_WEB_BIND"].rsplit(":", 1)
with http.server.ThreadingHTTPServer((host, int(port)), Handler) as server:
    server.daemon_threads = True
    print("auth fixture listening", flush=True)
    server.serve_forever()
