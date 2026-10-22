#!/usr/bin/env python3
"""C6 container acceptance using only generated containers/volumes and fixture credentials."""
import argparse
import base64
import hashlib
import hmac
import json
import socket
import subprocess
import time
import uuid
from pathlib import Path

from smoke_web_package import check_api, client, login


def docker(*args, input=None, check=True):
    result = subprocess.run(['docker', *args], input=input, text=True, encoding='utf-8',
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=120)
    if check and result.returncode:
        raise RuntimeError(f'docker {args[0]} failed: {result.stderr}')
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--image', required=True)
    parser.add_argument('--build-id', required=True)
    parser.add_argument('--package', required=True, type=Path)
    parser.add_argument('--logs', required=True, type=Path)
    args = parser.parse_args()
    args.logs.mkdir(parents=True, exist_ok=True)
    (args.logs / 'result.json').write_text('{"status":"fail","reason":"run not completed"}\n', encoding='utf-8')
    prefix = 'rssr-c6-' + uuid.uuid4().hex[:12]
    volumes, containers, checks = [], [], []

    def volume(suffix):
        name = prefix + '-' + suffix
        docker('volume', 'create', name)
        volumes.append(name)
        return name

    def start(name, data, legacy=None):
        with socket.socket() as listener:
            listener.bind(('127.0.0.1', 0))
            port = listener.getsockname()[1]
        origin = f'http://127.0.0.1:{port}'
        command = ['run', '-d', '--name', name, '-p', f'127.0.0.1:{port}:8080',
                   '-v', f'{data}:/data', '-e', 'RSS_READER_WEB_USERNAME=smoke',
                   '-e', 'RSS_READER_WEB_ENV=development',
                   '-e', f'RSS_READER_WEB_PUBLIC_ORIGIN={origin}']
        if legacy:
            command += ['-v', f'{legacy}:/app/auth:ro', '-e', 'RSS_READER_WEB_AUTH_STATE_FILE=/app/auth/auth.json']
        else:
            command += ['-e', 'RSS_READER_WEB_PASSWORD=package-smoke-pass']
        containers.append(name)
        docker(*command, args.image)
        wait(origin)
        return origin

    def wait(origin):
        deadline = time.monotonic() + 40
        while time.monotonic() < deadline:
            try:
                if client(origin)('/healthz')[0] == 200:
                    return
            except OSError:
                pass
            time.sleep(0.3)
        raise TimeoutError('container health check failed')

    try:
        data = volume('fresh-data')
        fresh = prefix + '-fresh'
        origin = start(fresh, data)
        check_api(origin, args.build_id)
        checks += ['new data volume, login and matching build write', 'old build rejected without mutation']
        metadata = json.loads((args.package / 'build.json').read_text())
        assert metadata['build_id'] == args.build_id
        actual_hash = docker('exec', fresh, 'sha256sum', '/usr/local/bin/rssr-web').stdout.split()[0]
        assert actual_hash == metadata['files']['rssr-web'], 'image rebuilt/replaced the packaged server'
        checks.append('image server is the exact packaged binary')
        docker('exec', fresh, 'sh', '-c', 'test -s /data/library.sqlite3 && test -s /data/auth.sqlite3 && test -s /data/outbound.json')
        docker('exec', '-u', 'rssr', fresh, 'rssr-cli', 'save-settings', '--theme', 'light')
        request = client(origin)
        login(request)
        assert json.loads(request('/api/v1/settings')[1])['theme'] == 'light'
        checks.append('owner CLI shares the running server library')
        for user in ['0', '65534']:
            result = docker('exec', '-u', user, fresh, 'rssr-cli', 'show-settings', check=False)
            assert result.returncode != 0, f'non-owner CLI was accepted: uid {user}'
            (args.logs / f'non-owner-{user}.log').write_text(result.stdout + result.stderr, encoding='utf-8')
        checks.append('CLI rejects root and unrelated uid for owner data')
        docker('exec', '-u', 'rssr', fresh, 'rssr-cli', 'backup', '/data/c6-backup.sqlite3')
        checks.append('owner CLI backup while server runs')
        docker('restart', '--time', '15', fresh)
        wait(origin)
        assert request('/api/v1/session')[0] == 200, 'graceful restart lost the session'
        assert json.loads(request('/api/v1/settings')[1])['theme'] == 'light', 'restart lost the CLI write'
        checks.append('graceful restart retains session and data')

        legacy = volume('legacy-auth')
        migrated = volume('migrated-data')
        password = 'legacy-smoke-pass'
        password_hash = docker('run', '--rm', args.image, 'rssr-web', '--print-password-hash', password).stdout.strip()
        secret = b'7' * 32
        # v0.1.22 persisted_state/session format; generated fixture, never a user's credential.
        old_state = json.dumps({'password_hash': password_hash, 'session_secret': secret.decode('ascii')})
        docker('run', '--rm', '-i', '-v', f'{legacy}:/app/auth', args.image,
               'sh', '-c', 'umask 077; cat > /app/auth/auth.json', input=old_state)
        upgrade = prefix + '-upgrade'
        upgraded_origin = start(upgrade, migrated, legacy)
        payload = f'smoke:{int(time.time()) + 3600}'.encode()
        encode = lambda value: base64.urlsafe_b64encode(value).decode().rstrip('=')
        old_cookie = encode(payload) + '.' + encode(hmac.new(secret, payload, hashlib.sha256).digest())
        status, _, _ = client(upgraded_origin)('/api/v1/session', headers={'Cookie': 'rssr_web_session=' + old_cookie})
        assert status == 401, 'legacy HMAC session was accepted'
        check_api(upgraded_origin, args.build_id, password)
        assert docker('exec', upgrade, 'cat', '/app/auth/auth.json').stdout == old_state
        checks.append('read-only legacy credential volume migrates unchanged; old session rejected')
        docker('stop', '--time', '15', upgrade)
        # A successful auth.sqlite3 is authoritative even after the old volume is detached and
        # an unrelated initial password is present. Only this generated fixture is restarted.
        detached = prefix + '-detached'
        detached_origin = start(detached, migrated)
        check_api(detached_origin, args.build_id, password)
        checks.append('migrated credentials survive removal of the legacy mount')
    finally:
        # Names are generated above and recorded only for this run; never enumerate user volumes.
        cleanup_errors = []
        for name in containers:
            try:
                result = docker('logs', name, check=False)
                (args.logs / (name + '.log')).write_text(result.stdout + result.stderr, encoding='utf-8')
            except Exception as error:
                cleanup_errors.append(str(error))
            try:
                docker('rm', '-f', name)
            except Exception as error:
                cleanup_errors.append(str(error))
        for name in volumes:
            try:
                docker('volume', 'rm', name)
            except Exception as error:
                cleanup_errors.append(str(error))
        if cleanup_errors:
            raise RuntimeError('container cleanup failed: ' + '; '.join(cleanup_errors))
    (args.logs / 'result.json').write_text(json.dumps({
        'status': 'pass', 'build_id': args.build_id, 'image': args.image, 'checks': checks,
    }, indent=2) + '\n', encoding='utf-8')
    print(f'C6 container acceptance passed: {len(checks)} checks')


if __name__ == '__main__':
    main()
