#!/usr/bin/env python3
from __future__ import annotations

import argparse
import shutil
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

# 严格三段式：v0.1.13 / 0.1.13。刻意不接受 -rc1、+build 之类后缀，理由见 parse_release_tag。
RELEASE_TAG_PATTERN = re.compile(r"^v?(\d+)\.(\d+)\.(\d+)$")


def parse_release_tag(tag: str) -> tuple[int, str]:
    """把发布 tag 解析成 `(versionCode, versionName)`。

    前置条件：`tag` 形如 `v0.1.13`，不带任何预发布 / 构建后缀。
    后置条件：versionCode 为正整数且随 tag 单调递增，versionName 为去掉前导 `v` 的原串。

    versionCode 用 `major * 10000 + minor * 100 + patch`。要求 minor / patch 都小于 100，
    否则单调性会被打破——`0.1.100` 与 `0.2.0` 会算出同一个 versionCode，之后发出去的包
    在设备上无法互相覆盖升级，而构建侧不会有任何报错。

    后缀被整个拒掉，有两个独立原因，任何一个都足以拒绝：

    1. `v0.1.13-rc1` 与 `v0.1.13` 会算出同一个 versionCode。侧载时相同 versionCode 可以
       覆盖安装，所以不会报错，但正式版在装过 rc 的设备上看起来「没有更新」；将来若上架
       应用商店（要求 versionCode 严格递增）会被直接拒绝。
    2. versionName 会被原样插进 Gradle Kotlin DSL 的字符串字面量。后缀若不加限制，
       tag 里的引号就能从字面量里逃出去，在 Gradle 配置阶段执行任意代码；含 `$` 的 tag
       还会被当成 Kotlin 字符串模板。
    """
    match = RELEASE_TAG_PATTERN.match(tag.strip())
    if match is None:
        raise ValueError(f"无法解析发布 tag: {tag!r}，期望形如 v0.1.13")

    major, minor, patch = (int(part) for part in match.groups())
    if minor >= 100 or patch >= 100:
        raise ValueError(f"minor / patch 必须小于 100，否则 versionCode 不再单调: {tag!r}")

    version_code = major * 10000 + minor * 100 + patch
    if version_code < 1:
        raise ValueError(f"versionCode 必须为正数，Android 不接受 0: {tag!r}")

    return (version_code, tag.strip().removeprefix("v"))


def patch_gradle_version(text: str, version_code: int, version_name: str) -> str:
    """把 versionCode / versionName 写进生成的 Gradle 文件。

    Dioxus 脚手架固定生成 `versionCode = 1` / `versionName = "0.1.0"`，而工作区
    `Cargo.toml` 的版本号长期停在 0.1.0，所以真正的版本号只能来自发布 tag。

    这里必须**验证替换确实发生**：脚手架换个写法（Kotlin DSL 与 Groovy 的赋值语法不同）
    就会让正则静默落空，产物照样带着 versionCode=1 发出去，而 CI 全绿。宁可在这里炸掉。
    """
    replacements = (
        (r"versionCode\s*=\s*\d+", f"versionCode = {version_code}"),
        (r"versionCode\s+\d+", f"versionCode {version_code}"),
        (r'versionName\s*=\s*"[^"]*"', f'versionName = "{version_name}"'),
        (r'versionName\s+"[^"]*"', f'versionName "{version_name}"'),
    )

    updated = text
    for pattern, replacement in replacements:
        updated = re.sub(pattern, replacement, updated)

    if not re.search(rf"versionCode\s*=?\s*{version_code}\b", updated):
        raise RuntimeError(f"未能写入 versionCode={version_code}，生成的 Gradle 文件写法可能变了")
    if not re.search(rf'versionName\s*=?\s*"{re.escape(version_name)}"', updated):
        raise RuntimeError(f"未能写入 versionName={version_name}，生成的 Gradle 文件写法可能变了")

    return updated


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


def patch_gradle_file(
    gradle_path: Path,
    min_sdk: int,
    target_sdk: int,
    version: tuple[int, str] | None = None,
) -> None:
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

    if version is not None:
        updated = patch_gradle_version(updated, version[0], version[1])

    if updated != text:
        gradle_path.write_text(updated, encoding="utf-8")


def patch_gradle(
    main_dir: Path,
    min_sdk: int,
    target_sdk: int,
    version: tuple[int, str] | None = None,
) -> None:
    app_module_root = main_dir.parent.parent
    candidates = [
        app_module_root / "build.gradle.kts",
        app_module_root / "build.gradle",
    ]

    patched = False
    for gradle_path in candidates:
        if gradle_path.exists():
            patch_gradle_file(gradle_path, min_sdk, target_sdk, version)
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
    parser = argparse.ArgumentParser(description="修补 dx 生成的 Android 工程资源与版本号")
    parser.add_argument("main_dir", help="生成的 Gradle 工程里的 app/src/main 目录")
    parser.add_argument(
        "--release-tag",
        default=None,
        help="发布 tag（如 v0.1.13）。给出时写入 versionCode / versionName；本地调试可省略",
    )
    args = parser.parse_args()

    main_dir = Path(args.main_dir).resolve()
    repo_root = Path(__file__).resolve().parent.parent
    min_sdk, target_sdk = load_android_sdk_config(repo_root)
    version = parse_release_tag(args.release_tag) if args.release_tag else None

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
    patch_gradle(main_dir, min_sdk, target_sdk, version)
    version_note = (
        f", versionCode={version[0]}, versionName={version[1]}" if version else ", version=未改动"
    )
    print(
        f"patched Android bundle resources under {main_dir} "
        f"(minSdk={min_sdk}, targetSdk={target_sdk}, compileSdk={target_sdk}{version_note})"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
