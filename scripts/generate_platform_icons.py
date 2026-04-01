#!/usr/bin/env python3
from __future__ import annotations

import sys
from pathlib import Path

from PIL import Image


ANDROID_SIZES = {
    "mipmap-mdpi": 48,
    "mipmap-hdpi": 72,
    "mipmap-xhdpi": 96,
    "mipmap-xxhdpi": 144,
    "mipmap-xxxhdpi": 192,
}

ICNS_SIZES = [16, 32, 64, 128, 256, 512, 1024]


def resized_square(source: Image.Image, size: int) -> Image.Image:
    icon = source.copy()
    icon.thumbnail((size, size), Image.Resampling.LANCZOS)
    canvas = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    offset = ((size - icon.width) // 2, (size - icon.height) // 2)
    canvas.paste(icon, offset, icon)
    return canvas


def write_android_icons(repo_root: Path, source: Image.Image) -> None:
    android_root = repo_root / "icons" / "android"
    for dirname, size in ANDROID_SIZES.items():
        out_dir = android_root / dirname
        out_dir.mkdir(parents=True, exist_ok=True)
        square = resized_square(source, size)
        square.save(out_dir / "rssr_launcher.png", format="PNG")
        square.save(out_dir / "rssr_launcher_round.png", format="PNG")


def write_icns(repo_root: Path, source: Image.Image) -> None:
    sizes = [(size, size) for size in ICNS_SIZES]
    iconset = [resized_square(source, size) for size in ICNS_SIZES]
    iconset[0].save(repo_root / "icons" / "icon.icns", format="ICNS", sizes=sizes, append_images=iconset[1:])


def main() -> int:
    repo_root = Path(__file__).resolve().parent.parent
    source_path = repo_root / "icons" / "icon.png"
    if not source_path.exists():
        print(f"missing source icon: {source_path}", file=sys.stderr)
        return 1

    source = Image.open(source_path).convert("RGBA")
    write_android_icons(repo_root, source)
    write_icns(repo_root, source)
    print("generated Android launcher icons and icon.icns")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
