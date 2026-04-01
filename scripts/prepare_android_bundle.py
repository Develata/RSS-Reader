#!/usr/bin/env python3
from __future__ import annotations

import shutil
import sys
import xml.etree.ElementTree as ET
from pathlib import Path


ANDROID_NS = "http://schemas.android.com/apk/res/android"
ET.register_namespace("android", ANDROID_NS)

APP_NAME = "RSS-Reader"
ICON_NAME = "rssr_launcher"
ROUND_ICON_NAME = "rssr_launcher_round"


def android_attr(name: str) -> str:
    return f"{{{ANDROID_NS}}}{name}"


def patch_manifest(manifest_path: Path) -> None:
    tree = ET.parse(manifest_path)
    root = tree.getroot()
    application = root.find("application")
    if application is None:
        raise RuntimeError(f"missing <application> in {manifest_path}")

    application.set(android_attr("icon"), f"@mipmap/{ICON_NAME}")
    application.set(android_attr("roundIcon"), f"@mipmap/{ROUND_ICON_NAME}")

    tree.write(manifest_path, encoding="utf-8", xml_declaration=True)


def patch_strings(strings_path: Path) -> None:
    tree = ET.parse(strings_path)
    root = tree.getroot()

    app_name_node = None
    for string_node in root.findall("string"):
        if string_node.get("name") == "app_name":
            app_name_node = string_node
            break

    if app_name_node is None:
        app_name_node = ET.SubElement(root, "string", {"name": "app_name"})

    app_name_node.text = APP_NAME
    tree.write(strings_path, encoding="utf-8", xml_declaration=True)


def copy_launcher_icons(repo_root: Path, res_dir: Path) -> None:
    source_root = repo_root / "icons" / "android"
    mipmaps = ["mipmap-mdpi", "mipmap-hdpi", "mipmap-xhdpi", "mipmap-xxhdpi", "mipmap-xxxhdpi"]

    for mipmap in mipmaps:
        source_dir = source_root / mipmap
        target_dir = res_dir / mipmap
        target_dir.mkdir(parents=True, exist_ok=True)

        for filename in (f"{ICON_NAME}.png", f"{ROUND_ICON_NAME}.png"):
            source_path = source_dir / filename
            if not source_path.exists():
                raise RuntimeError(f"missing source icon {source_path}")
            shutil.copy2(source_path, target_dir / filename)


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: prepare_android_bundle.py <android-app-src-main-dir>", file=sys.stderr)
        return 1

    main_dir = Path(sys.argv[1]).resolve()
    repo_root = Path(__file__).resolve().parent.parent

    manifest_path = main_dir / "AndroidManifest.xml"
    strings_path = main_dir / "res" / "values" / "strings.xml"
    res_dir = main_dir / "res"

    if not manifest_path.exists():
        raise RuntimeError(f"manifest not found: {manifest_path}")
    if not strings_path.exists():
        raise RuntimeError(f"strings.xml not found: {strings_path}")

    copy_launcher_icons(repo_root, res_dir)
    patch_manifest(manifest_path)
    patch_strings(strings_path)
    print(f"patched Android bundle resources under {main_dir}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
