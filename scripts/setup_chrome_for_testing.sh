#!/usr/bin/env bash
set -euo pipefail

version="${1:-${CHROME_FOR_TESTING_VERSION:-stable}}"
install_root="${2:-${HOME}/.cache/chrome-for-testing}"
bin_dir="${3:-${HOME}/.local/bin}"
with_driver="${4:-true}"
platform="linux64"

resolve_stable_version() {
  python3 - <<'PY'
import json
import urllib.request

with urllib.request.urlopen(
    "https://googlechromelabs.github.io/chrome-for-testing/last-known-good-versions-with-downloads.json",
    timeout=30,
) as response:
    payload = json.load(response)

print(payload["channels"]["Stable"]["version"])
PY
}

download_and_extract() {
  local url="$1"
  local dest_dir="$2"
  local zip_path="$3"

  mkdir -p "$dest_dir"
  curl -L --fail --retry 3 --retry-delay 1 --connect-timeout 15 --max-time 180 -o "$zip_path" "$url"
  python3 - "$zip_path" "$dest_dir" <<'PY'
import sys
import zipfile

archive_path, dest_dir = sys.argv[1], sys.argv[2]
with zipfile.ZipFile(archive_path) as archive:
    archive.extractall(dest_dir)
PY
  rm -f "$zip_path"
}

if [[ "$version" == "--resolve-version" ]]; then
  resolve_stable_version
  exit 0
fi
if [[ "$with_driver" != true && "$with_driver" != false ]]; then
  echo "with-driver must be true or false" >&2
  exit 1
fi
if [[ "$version" == "stable" ]]; then
  version="$(resolve_stable_version)"
fi
if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "Invalid Chrome for Testing version: $version" >&2
  exit 1
fi

version_dir="${install_root}/${version}"
chrome_dir="${version_dir}/chrome-${platform}"
driver_dir="${version_dir}/chromedriver-${platform}"

mkdir -p "$install_root" "$bin_dir"

if [[ ! -x "${chrome_dir}/chrome" ]]; then
  download_and_extract \
    "https://storage.googleapis.com/chrome-for-testing-public/${version}/${platform}/chrome-${platform}.zip" \
    "$version_dir" \
    "${version_dir}/chrome-${platform}.zip"
fi

if [[ "$with_driver" == true && ! -x "${driver_dir}/chromedriver" ]]; then
  download_and_extract \
    "https://storage.googleapis.com/chrome-for-testing-public/${version}/${platform}/chromedriver-${platform}.zip" \
    "$version_dir" \
    "${version_dir}/chromedriver-${platform}.zip"
fi

find "${chrome_dir}" -maxdepth 1 -type f -exec chmod +x {} +

ln -sf "${chrome_dir}/chrome" "${bin_dir}/google-chrome"
ln -sf "${chrome_dir}/chrome" "${bin_dir}/google-chrome-stable"

echo "Installed Chrome for Testing ${version}"
"${bin_dir}/google-chrome" --version
if [[ "$with_driver" == true ]]; then
  find "${driver_dir}" -maxdepth 1 -type f -exec chmod +x {} +
  ln -sf "${driver_dir}/chromedriver" "${bin_dir}/chromedriver"
  "${bin_dir}/chromedriver" --version
fi
