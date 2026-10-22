#!/usr/bin/env python3
"""Run an extracted Web distribution on an isolated library and check its build contract."""
import argparse
import contextlib
import http.cookiejar
import json
import os
import socket
import subprocess
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

from package_web import verify


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *args, **kwargs):
        return None


def client(origin):
    cookies = http.cookiejar.CookieJar()
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}),
                                         urllib.request.HTTPCookieProcessor(cookies), NoRedirect())

    def request(path, method='GET', body=None, headers=None):
        request_headers = {'Origin': origin, **(headers or {})}
        if isinstance(body, dict):
            body = json.dumps(body).encode()
            request_headers['Content-Type'] = 'application/json'
        request = urllib.request.Request(origin + path, data=body, method=method, headers=request_headers)
        try:
            response = opener.open(request, timeout=5)
        except urllib.error.HTTPError as error:
            response = error
        with response:
            return response.status, response.read(), dict(response.headers)
    return request


def login(request, password='package-smoke-pass'):
    body = urllib.parse.urlencode({'username': 'smoke', 'password': password, 'next': '/'}).encode()
    status, _, _ = request('/login', 'POST', body, {'Content-Type': 'application/x-www-form-urlencoded'})
    assert status == 303, f'login returned {status}'
    status, body, _ = request('/api/v1/session')
    assert status == 200, (status, body)
    return json.loads(body)['csrf_token']


def check_api(origin, identifier, password='package-smoke-pass'):
    request = client(origin)
    token = login(request, password)
    status, body, _ = request('/')
    assert status == 200 and b'<html' in body.lower(), 'packaged index is not served'
    headers = {'X-RSSR-CSRF': token, 'X-RSSR-Build': identifier}
    status, body, _ = request('/api/v1/settings', 'PATCH', {'theme': 'dark'}, headers)
    assert status == 200, (status, body)
    status, body, _ = request('/api/v1/settings')
    assert status == 200 and json.loads(body)['theme'] == 'dark', (status, body)
    status, body, _ = request('/api/v1/settings', 'PATCH', {'theme': 'light'},
                              {**headers, 'X-RSSR-Build': identifier + '-old'})
    assert status == 409 and json.loads(body)['code'] == 'build_mismatch', (status, body)
    status, body, _ = request('/api/v1/settings')
    assert status == 200 and json.loads(body)['theme'] == 'dark', 'old page changed the library'


@contextlib.contextmanager
def server(root, data, log, *, public=None, password='package-smoke-pass'):
    metadata = verify(root)
    with socket.socket() as listener:
        listener.bind(('127.0.0.1', 0))
        port = listener.getsockname()[1]
    origin = f'http://127.0.0.1:{port}'
    env = {k: v for k, v in os.environ.items() if not k.startswith(('RSSR_', 'RSS_READER_'))}
    env.update(RSSR_DATA_DIR=str(data), RSS_READER_WEB_BIND=f'127.0.0.1:{port}',
               RSS_READER_WEB_STATIC_DIR=str(public or root / 'public'),
               RSS_READER_WEB_PUBLIC_ORIGIN=origin, RSS_READER_WEB_ENV='development',
               RSS_READER_WEB_USERNAME='smoke', RSS_READER_WEB_PASSWORD=password)
    with log.open('w', encoding='utf-8') as output:
        process = subprocess.Popen([str(root / metadata['binary'])], env=env, stdout=output, stderr=subprocess.STDOUT)
        try:
            deadline = time.monotonic() + 30
            while time.monotonic() < deadline:
                if process.poll() is not None:
                    raise RuntimeError(log.read_text(encoding='utf-8', errors='replace'))
                try:
                    if client(origin)('/healthz')[0] == 200:
                        break
                except OSError:
                    pass
                time.sleep(0.2)
            else:
                raise TimeoutError('packaged server did not start')
            yield origin
        finally:
            if process.poll() is None:
                process.terminate()
            try:
                process.wait(timeout=15)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root', type=Path)
    parser.add_argument('--logs', required=True, type=Path)
    parser.add_argument('--chrome', type=Path, help='Also drive the actual page in this Chrome executable')
    parser.add_argument('--old-package', type=Path, help='Serve this different-build page with the current server')
    args = parser.parse_args()
    root = args.root.resolve()
    args.logs.mkdir(parents=True, exist_ok=True)
    (args.logs / 'result.json').write_text('{"status":"fail","reason":"run not completed"}\n', encoding='utf-8')
    metadata = verify(root)
    browser_checks = []
    with tempfile.TemporaryDirectory(prefix='rssr-web-smoke-') as directory:
        # macOS temp paths use /var -> /private/var. Auth SQLite intentionally opens
        # with NOFOLLOW, so pass the real path of this owned test directory.
        scratch = Path(directory).resolve()
        with server(root, scratch / 'data', args.logs / 'server.log') as origin:
            check_api(origin, metadata['build_id'])
            if args.chrome:
                browser_smoke(args.chrome, origin, metadata['build_id'], 'matching', args.logs)
                browser_checks.append('matching real page writes with its compiled build id')
        if args.old_package:
            if not args.chrome:
                raise ValueError('--old-package requires --chrome')
            old = args.old_package.resolve()
            old_id = verify(old)['build_id']
            assert old_id != metadata['build_id'], 'old page must have a different compiled identifier'
            with server(root, scratch / 'old-data', args.logs / 'old-page-server.log', public=old / 'public') as origin:
                browser_smoke(args.chrome, origin, old_id, 'old-page', args.logs)
                browser_checks.append('actual older page is rejected and shows recovery without a write')
    (args.logs / 'result.json').write_text(json.dumps({
        'status': 'pass', 'build_id': metadata['build_id'],
        'checks': ['package hashes and WASM id', 'new library', 'login', 'static index',
                   'matching build writes', 'old build rejected without mutation'] + browser_checks,
    }, indent=2) + '\n', encoding='utf-8')
    print('Web package smoke passed')


def browser_smoke(chrome, origin, identifier, mode, logs):
    with socket.socket() as listener:
        listener.bind(('127.0.0.1', 0))
        port = listener.getsockname()[1]
    cdp = f'http://127.0.0.1:{port}'
    with tempfile.TemporaryDirectory(prefix='rssr-package-chrome-') as profile:
        with (logs / f'{mode}-chrome.log').open('w', encoding='utf-8') as output:
            process = subprocess.Popen([str(chrome), '--headless=new', '--disable-gpu',
                '--disable-background-networking', '--no-first-run', '--no-default-browser-check',
                '--no-proxy-server', '--remote-allow-origins=*', f'--remote-debugging-port={port}',
                f'--user-data-dir={profile}', 'about:blank'], stdout=output, stderr=subprocess.STDOUT)
            try:
                deadline = time.monotonic() + 20
                while time.monotonic() < deadline:
                    try:
                        if client(cdp)('/json/version')[0] == 200:
                            break
                    except OSError:
                        pass
                    time.sleep(0.2)
                else:
                    raise TimeoutError('package browser did not start')
                driver = Path(__file__).parent / 'browser/web_package_smoke.mjs'
                subprocess.run(['node', str(driver), cdp, origin, identifier, mode,
                                str(logs / f'{mode}-browser.json')], check=True, timeout=90)
            finally:
                try:
                    process.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    process.terminate()
                    process.wait(timeout=10)


if __name__ == '__main__':
    main()
