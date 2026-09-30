#!/usr/bin/env python3
"""Package an existing dx bundle; never generate an alternative application UI.

--serve emulates Pages' HTTP 404 fallback for local acceptance, without seed helpers.
"""
import argparse
import http.server
from pathlib import Path
import shutil
from urllib.parse import unquote, urlsplit


def prepare(public):
    index = public / "index.html"
    if not index.is_file() or not any(public.rglob("*.wasm")):
        raise SystemExit("Expected a real dx Web bundle with index.html and WASM")
    shutil.copyfile(index, public / "404.html")
    (public / ".nojekyll").touch()


def serve(public, base, port, port_file=None):
    prefix = "/" + base.strip("/") if base.strip("/") else ""

    class PagesHandler(http.server.SimpleHTTPRequestHandler):
        def do_GET(self):
            path = unquote(urlsplit(self.path).path)
            if path == prefix and prefix:
                self.send_response(301)
                self.send_header("Location", prefix + "/")
                self.end_headers()
                return
            if not path.startswith(prefix + "/"):
                self.send_error(404)
                return
            relative = path[len(prefix):].lstrip("/")
            target = (public / relative).resolve()
            if not target.is_relative_to(public):
                self.send_error(404)
                return
            if target.is_dir():
                target /= "index.html"
            if target.is_file():
                self.path = "/" + relative
                return super().do_GET()
            body = (public / "404.html").read_bytes()
            self.send_response(404)
            self.send_header("Content-Type", "text/html; charset=utf-8")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

    handler = lambda *args, **kwargs: PagesHandler(*args, directory=str(public), **kwargs)
    with http.server.ThreadingHTTPServer(("127.0.0.1", port), handler) as server:
        if port_file is not None:
            port_file.write_text(f"{server.server_port}\n", encoding="utf-8")
        server.serve_forever()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("public", type=Path)
    parser.add_argument("--serve", action="store_true")
    parser.add_argument("--base-path", default="/RSS-Reader")
    parser.add_argument("--port", type=int, default=8093)
    parser.add_argument("--port-file", type=Path)
    args = parser.parse_args()
    public = args.public.resolve()
    if args.serve:
        if not (public / "index.html").is_file() or not (public / "404.html").is_file():
            raise SystemExit("Prepare the Pages bundle before serving it")
        serve(public, args.base_path, args.port, args.port_file)
    else:
        prepare(public)
