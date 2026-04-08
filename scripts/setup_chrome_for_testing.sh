#!/usr/bin/env bash
set -euo pipefail

version="${1:-${CHROME_FOR_TESTING_VERSION:-stable}}"
install_root="${2:-${HOME}/.cache/chrome-for-testing}"
bin_dir="${3:-${HOME}/.local/bin}"
platform="linux64"

resolve_stable_version() {
  python3 - <<'PY'
import json
import urllib.request

with urllib.request.urlopen(
    "https://googlechromelabs.github.io/chrome-for-testing/last-known-good-versions-with-downloads.json"
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
  curl -L --fail --retry 3 --retry-delay 1 -o "$zip_path" "$url"
  python3 - "$zip_path" "$dest_dir" <<'PY'
import sys
import zipfile

archive_path, dest_dir = sys.argv[1], sys.argv[2]
with zipfile.ZipFile(archive_path) as archive:
    archive.extractall(dest_dir)
PY
  rm -f "$zip_path"
}

if [[ "$version" == "stable" ]]; then
  version="$(resolve_stable_version)"
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

if [[ ! -x "${driver_dir}/chromedriver" ]]; then
  download_and_extract \
    "https://storage.googleapis.com/chrome-for-testing-public/${version}/${platform}/chromedriver-${platform}.zip" \
    "$version_dir" \
    "${version_dir}/chromedriver-${platform}.zip"
fi

ln -sf "${chrome_dir}/chrome" "${bin_dir}/google-chrome"
ln -sf "${chrome_dir}/chrome" "${bin_dir}/google-chrome-stable"
ln -sf "${driver_dir}/chromedriver" "${bin_dir}/chromedriver"

chmod +x "${chrome_dir}/chrome" "${driver_dir}/chromedriver"

echo "Installed Chrome for Testing ${version}"
"${bin_dir}/google-chrome" --version
"${bin_dir}/chromedriver" --version
