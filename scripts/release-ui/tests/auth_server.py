"""Local HTTP peer for web-auth lifecycle and unchanged shell assertion contracts."""
import http.server
import json
import os
import signal
from pathlib import Path
import subprocess
import sys
import threading
import time
import urllib.parse

if "--leaf" in sys.argv:
    if os.environ.get("AUTH_STUBBORN_LEAF"):
        signal.signal(signal.SIGTERM, signal.SIG_IGN)
        signal.signal(signal.SIGINT, signal.SIG_IGN)
    Path("auth-leaf-ready").touch()
    time.sleep(300)
    raise SystemExit()

lock = threading.Lock()
health_count = 0
# Per-session state, as the real server keeps it: live tokens with their CSRF token, and
# revoked tokens (a replayed revoked cookie is refused like an expired session).
live = {}
revoked = set()
issued = 0
Path("auth-leaf-ready").unlink(missing_ok=True)
leaf = subprocess.Popen([sys.executable, __file__, "--leaf"])
deadline = time.monotonic() + 5
while not Path("auth-leaf-ready").exists():
    if time.monotonic() > deadline or leaf.poll() is not None:
        raise RuntimeError("auth leaf failed to start")
    time.sleep(.01)
Path("auth-pids.json").write_text(json.dumps([os.getpid(), leaf.pid]), encoding="utf-8")


class Handler(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def reply(self, status, headers=(), body=b""):
        self.send_response(status)
        for key, value in headers:
            self.send_header(key, value)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def reply_json(self, status, value):
        self.reply(status, [("Content-Type", "application/json"), ("Cache-Control", "no-store")],
                   json.dumps(value).encode())

    def trusted_origin(self):
        return self.headers.get("Origin") == f"http://127.0.0.1:{port}"

    def session(self):
        for pair in self.headers.get("Cookie", "").split(";"):
            name, _, value = pair.strip().partition("=")
            if name == "rssr_web_session":
                return value
        return None

    def do_GET(self):
        global health_count
        with lock:
            with Path("auth-requests.jsonl").open("a", encoding="utf-8") as out:
                out.write(json.dumps({"method": "GET", "path": self.path,
                                      "cookie": self.headers.get("Cookie", "")}) + "\n")
            if self.path == "/healthz":
                health_count += 1
                count = health_count
            token = self.session()
            csrf = live.get(token)
            was_revoked = token in revoked
        if self.path == "/healthz":
            if count <= int(os.environ.get("AUTH_SLOW_ATTEMPTS", "0")):
                time.sleep(float(os.environ.get("AUTH_DELAY", "11")))
            self.reply(200)
        elif self.path == "/login":
            self.reply(200)
        elif self.path == "/entries":
            self.reply(302, [("Location", "/login")])
        elif self.path == "/api/v1/session":
            if csrf is None:
                self.reply_json(401, {"code": "unauthenticated", "message": "fixture"})
            else:
                self.reply_json(200, {"csrf_token": csrf})
        elif self.path in ["/session-probe", "/feeds", "/settings", "/logout"]:
            if csrf is None:
                error = "error=session_expired&" if was_revoked or token else ""
                self.reply(302, [("Location", f"/login?{error}next=%2F")])
                return
            if os.environ.get("AUTH_EXIT_DURING_ASSERTIONS") and self.path == "/feeds":
                os._exit(17)
            if self.path == "/logout":
                self.reply(200, [("Cache-Control", "no-store")],
                           f'<form method="post" action="/logout"><input type="hidden" '
                           f'name="csrf" value="{csrf}"></form>'.encode())
            else:
                self.reply(204 if self.path == "/session-probe" else 200)
        else:
            self.reply(404)

    def do_POST(self):
        global issued
        body = self.rfile.read(int(self.headers["Content-Length"])).decode("ascii")
        form = urllib.parse.parse_qs(body)
        with lock:
            with Path("auth-requests.jsonl").open("a", encoding="utf-8") as out:
                out.write(json.dumps({"method": "POST", "path": self.path, "form": form,
                                      "origin": self.headers.get("Origin")}) + "\n")
        if not self.trusted_origin():
            self.reply(403)
            return
        if self.path == "/logout":
            token = self.session()
            with lock:
                if token not in live or form != {"csrf": [live[token]]}:
                    self.reply(403)
                    return
                del live[token]
                revoked.add(token)
            self.reply(303, [("Location", "/login"),
                             ("Set-Cookie", "rssr_web_session=deleted; Path=/; HttpOnly; SameSite=Lax; Max-Age=0"),
                             ("Set-Cookie", "rssr_web_gate=deleted; Path=/; SameSite=Lax; Max-Age=0")])
            return
        if self.path != "/login" or form != {
            "username": ["smoke"], "password": ["smoke-pass-123"], "next": ["/feeds"]}:
            self.reply(422)
            return
        with lock:
            issued += 1
            token = f"fixture-token-{issued}"
            live[token] = f"fixture-csrf-{issued}"
        self.reply(303, [
            ("Location", "/wrong" if os.environ.get("AUTH_BAD_REDIRECT") else "/feeds"),
            ("Set-Cookie", f"rssr_web_session={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age=15552000"),
            ("Set-Cookie", "rssr_web_gate=1; Path=/; SameSite=Lax; Max-Age=15552000"),
        ])


host, port = os.environ["RSS_READER_WEB_BIND"].rsplit(":", 1)
with http.server.ThreadingHTTPServer((host, int(port)), Handler) as server:
    server.daemon_threads = True
    print("auth fixture listening", flush=True)
    server.serve_forever()
