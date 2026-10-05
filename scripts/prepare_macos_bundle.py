#!/usr/bin/env python3
"""Set release versions on an unsigned macOS app before archiving/signing."""

import argparse
from pathlib import Path
import plistlib
import re


VERSION_KEYS = ("CFBundleShortVersionString", "CFBundleVersion")


def prepare_bundle(bundle: Path, release_tag: str) -> str:
    # Only stable, canonical numeric tags fit both bundle version fields.
    match = re.fullmatch(r"v((?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*))", release_tag)
    if match is None:
        raise ValueError("release tag must be a canonical vX.Y.Z (no suffix or leading zeros)")
    version = match[1]

    contents = bundle / "Contents"
    plist_path = contents / "Info.plist"
    if bundle.suffix != ".app" or not bundle.is_dir() or not plist_path.is_file():
        raise ValueError(f"expected an app bundle with Contents/Info.plist: {bundle}")
    if plist_path.is_symlink():
        raise ValueError("refusing to replace a symlinked Info.plist")

    # dx currently leaves our bundle unsigned. Never invalidate a resource seal
    # if signing is added upstream: move this step before signing instead.
    for signature in (contents / "_CodeSignature", contents / "CodeResources"):
        if signature.exists() or signature.is_symlink():
            raise ValueError("bundle is already signed; set release versions before signing")

    original = plist_path.read_bytes()
    metadata = plistlib.loads(original)
    if not isinstance(metadata, dict) or any(
        not isinstance(metadata.get(key), str) for key in VERSION_KEYS
    ):
        raise ValueError("Info.plist must contain both bundle version fields as strings")
    for key in VERSION_KEYS:
        metadata[key] = version
    fmt = plistlib.FMT_BINARY if original.startswith(b"bplist00") else plistlib.FMT_XML
    plist_path.write_bytes(plistlib.dumps(metadata, fmt=fmt, sort_keys=False))
    return version


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("bundle", type=Path)
    parser.add_argument("--release-tag", required=True)
    args = parser.parse_args()
    try:
        version = prepare_bundle(args.bundle, args.release_tag)
    except (OSError, ValueError, plistlib.InvalidFileException) as error:
        parser.exit(1, f"macOS bundle metadata: {error}\n")
    print(f"{args.bundle}: both bundle versions set to {version}")


if __name__ == "__main__":
    main()
