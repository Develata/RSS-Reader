#!/usr/bin/env python3
"""Archive/extract a matched rssr-web + public distribution; no user data or dependencies."""
import argparse
import hashlib
import json
import shutil
import tarfile
import tempfile
import zipfile
from pathlib import Path, PurePosixPath


def sha256(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def build_id(value):
    if not value or value == 'dev-unknown' or any(not 33 <= ord(c) <= 126 for c in value):
        raise ValueError('an explicit printable RSSR_BUILD_ID is required (not dev-unknown)')
    return value


def verify(root):
    metadata = json.loads((root / 'build.json').read_text(encoding='utf-8'))
    build_id(metadata['build_id'])
    expected = metadata['files']
    actual = {p.relative_to(root).as_posix(): p for p in root.rglob('*') if p.is_file()}
    if any(p.is_symlink() for p in root.rglob('*')):
        raise ValueError('symlinks are not allowed in a web package')
    if set(actual) != set(expected) | {'build.json'}:
        raise ValueError('package file inventory does not match build.json')
    for name, digest in expected.items():
        if sha256(actual[name]) != digest:
            raise ValueError(f'package hash mismatch: {name}')
    if metadata['binary'] not in expected or 'public/index.html' not in expected:
        raise ValueError('package must contain its server and public/index.html')
    wasm = [p for name, p in actual.items() if name.startswith('public/') and name.endswith('.wasm')]
    if not wasm or not any(metadata['build_id'].encode('ascii') in p.read_bytes() for p in wasm):
        raise ValueError('page WASM does not contain the expected build identifier')
    return metadata


def create(binary, public, identifier, output):
    identifier = build_id(identifier)
    if binary.name not in ('rssr-web', 'rssr-web.exe') or not binary.is_file():
        raise ValueError('expected a built rssr-web executable')
    if not (public / 'index.html').is_file():
        raise ValueError('expected the complete public/ bundle')
    if output.exists():
        raise FileExistsError(output)
    output.parent.mkdir(parents=True, exist_ok=True)
    # This owned temporary directory contains only copies of explicit build outputs.
    with tempfile.TemporaryDirectory(prefix='rssr-web-package-') as directory:
        root = Path(directory)
        shutil.copy2(binary, root / binary.name)
        shutil.copytree(public, root / 'public', symlinks=True)
        files = {p.relative_to(root).as_posix(): sha256(p)
                 for p in sorted(root.rglob('*')) if p.is_file() and not p.is_symlink()}
        (root / 'build.json').write_text(json.dumps({
            'build_id': identifier, 'binary': binary.name, 'files': files,
        }, indent=2) + '\n', encoding='utf-8')
        verify(root)
        if output.suffix == '.zip':
            with zipfile.ZipFile(output, 'x', zipfile.ZIP_DEFLATED) as archive:
                for path in sorted(root.rglob('*')):
                    if path.is_file():
                        archive.write(path, path.relative_to(root).as_posix())
        elif output.name.endswith('.tar.gz'):
            with tarfile.open(output, 'x:gz') as archive:
                for path in sorted(root.iterdir()):
                    archive.add(path, arcname=path.name)
        else:
            raise ValueError('output must end in .zip or .tar.gz')
    print(f'{sha256(output)}  {output.name}')


def unpack(archive_path, destination):
    if destination.exists():
        raise FileExistsError(destination)
    destination.mkdir(parents=True)

    def safe(name):
        path = PurePosixPath(name)
        if '\\' in name or ':' in name or path.is_absolute() or '..' in path.parts:
            raise ValueError(f'unsafe archive path: {name}')
        return destination.joinpath(*path.parts)

    if archive_path.suffix == '.zip':
        with zipfile.ZipFile(archive_path) as archive:
            seen = set()
            for item in archive.infolist():
                output = safe(item.filename)
                if item.filename in seen or (item.external_attr >> 16) & 0o170000 == 0o120000:
                    raise ValueError(f'duplicate or symlink entry: {item.filename}')
                seen.add(item.filename)
                if item.is_dir():
                    output.mkdir(parents=True, exist_ok=True)
                else:
                    output.parent.mkdir(parents=True, exist_ok=True)
                    with archive.open(item) as source, output.open('xb') as target:
                        shutil.copyfileobj(source, target)
    else:
        with tarfile.open(archive_path) as archive:
            seen = set()
            for item in archive:
                output = safe(item.name)
                if item.name in seen or not (item.isdir() or item.isfile()):
                    raise ValueError(f'duplicate or non-regular entry: {item.name}')
                seen.add(item.name)
                if item.isdir():
                    output.mkdir(parents=True, exist_ok=True)
                else:
                    output.parent.mkdir(parents=True, exist_ok=True)
                    with archive.extractfile(item) as source, output.open('xb') as target:
                        shutil.copyfileobj(source, target)
                    output.chmod(item.mode & 0o777)
    metadata = verify(destination)
    print(f'verified {len(metadata["files"])} files, build {metadata["build_id"]}')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='command', required=True)
    pack = sub.add_parser('create')
    pack.add_argument('--binary', required=True, type=Path)
    pack.add_argument('--public', required=True, type=Path)
    pack.add_argument('--build-id', required=True)
    pack.add_argument('--output', required=True, type=Path)
    extract = sub.add_parser('unpack')
    extract.add_argument('archive', type=Path)
    extract.add_argument('destination', type=Path)
    args = parser.parse_args()
    if args.command == 'create':
        create(args.binary, args.public, args.build_id, args.output)
    else:
        unpack(args.archive, args.destination)


if __name__ == '__main__':
    main()
