#!/usr/bin/env python3
from __future__ import annotations

import shutil
import sys
import xml.etree.ElementTree as ET
from pathlib import Path
import re

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover
    import tomli as tomllib


ANDROID_NS = "http://schemas.android.com/apk/res/android"
ET.register_namespace("android", ANDROID_NS)

APP_NAME = "RSS-Reader"
ICON_NAME = "rssr_launcher"
ROUND_ICON_NAME = "rssr_launcher_round"
DEFAULT_MIN_SDK = 24
DEFAULT_TARGET_SDK = 34


def android_attr(name: str) -> str:
    return f"{{{ANDROID_NS}}}{name}"


def load_android_sdk_config(repo_root: Path) -> tuple[int, int]:
    config_path = repo_root / "Dioxus.toml"
    if not config_path.exists():
        return (DEFAULT_MIN_SDK, DEFAULT_TARGET_SDK)

    with config_path.open("rb") as handle:
        parsed = tomllib.load(handle)

    android = parsed.get("android", {})
    min_sdk = int(android.get("min_sdk", DEFAULT_MIN_SDK))
    target_sdk = int(android.get("target_sdk", DEFAULT_TARGET_SDK))
    return (min_sdk, target_sdk)


def patch_gradle_file(gradle_path: Path, min_sdk: int, target_sdk: int) -> None:
    text = gradle_path.read_text(encoding="utf-8")

    replacements = {
        r"compileSdk\s*=\s*\d+": f"compileSdk = {target_sdk}",
        r"minSdk\s*=\s*\d+": f"minSdk = {min_sdk}",
        r"targetSdk\s*=\s*\d+": f"targetSdk = {target_sdk}",
        r"compileSdkVersion\s+\d+": f"compileSdkVersion {target_sdk}",
        r"minSdkVersion\s+\d+": f"minSdkVersion {min_sdk}",
        r"targetSdkVersion\s+\d+": f"targetSdkVersion {target_sdk}",
    }

    updated = text
    for pattern, replacement in replacements.items():
        updated = re.sub(pattern, replacement, updated)

    if updated != text:
        gradle_path.write_text(updated, encoding="utf-8")


def patch_gradle(main_dir: Path, min_sdk: int, target_sdk: int) -> None:
    app_module_root = main_dir.parent.parent
    candidates = [
        app_module_root / "build.gradle.kts",
        app_module_root / "build.gradle",
    ]

    patched = False
    for gradle_path in candidates:
        if gradle_path.exists():
            patch_gradle_file(gradle_path, min_sdk, target_sdk)
            patched = True

    if not patched:
        raise RuntimeError(f"android Gradle file not found under {app_module_root}")


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
    min_sdk, target_sdk = load_android_sdk_config(repo_root)

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
    patch_gradle(main_dir, min_sdk, target_sdk)
    print(
        f"patched Android bundle resources under {main_dir} "
        f"(minSdk={min_sdk}, targetSdk={target_sdk}, compileSdk={target_sdk})"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
