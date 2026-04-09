#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 ]]; then
  echo "usage: $0 <harness-name>" >&2
  exit 1
fi

harness_name="$1"
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
crate_root="${repo_root}/crates/rssr-infra"
export PATH="${HOME}/.local/bin:${HOME}/.cargo/bin:${PATH}"

cd "$repo_root"

if ! command -v wasm-bindgen-test-runner >/dev/null 2>&1; then
  echo "wasm-bindgen-test-runner is required in PATH" >&2
  exit 1
fi

if ! command -v chromedriver >/dev/null 2>&1; then
  echo "chromedriver is required in PATH" >&2
  exit 1
fi

if command -v google-chrome >/dev/null 2>&1; then
  export BROWSER="${BROWSER:-google-chrome}"
fi

cargo test -p rssr-infra --target wasm32-unknown-unknown --test "${harness_name}" --no-run

wasm_artifact="$(
  find target/wasm32-unknown-unknown/debug/deps \
    -maxdepth 1 \
    -name "${harness_name}-*.wasm" \
    -printf '%T@ %p\n' \
    | sort -nr \
    | head -n1 \
    | cut -d' ' -f2-
)"

if [[ -z "${wasm_artifact}" ]]; then
  echo "failed to locate ${harness_name} wasm artifact" >&2
  exit 1
fi

webdriver_config="$(mktemp)"
profile_dir="$(mktemp -d)"
cleanup() {
  rm -f "${webdriver_config}"
  rm -rf "${profile_dir}"
}
trap cleanup EXIT

chrome_binary=""
if command -v google-chrome >/dev/null 2>&1; then
  chrome_binary="$(command -v google-chrome)"
fi

if [[ -n "${chrome_binary}" ]]; then
  cat >"${webdriver_config}" <<EOF
{
  "goog:chromeOptions": {
    "binary": "${chrome_binary}",
    "args": [
      "--headless=new",
      "--no-sandbox",
      "--disable-dev-shm-usage",
      "--disable-gpu",
      "--remote-allow-origins=*",
      "--window-size=1280,720",
      "--user-data-dir=${profile_dir}"
    ]
  }
}
EOF
else
  cat >"${webdriver_config}" <<EOF
{
  "goog:chromeOptions": {
    "args": [
      "--headless=new",
      "--no-sandbox",
      "--disable-dev-shm-usage",
      "--disable-gpu",
      "--remote-allow-origins=*",
      "--window-size=1280,720",
      "--user-data-dir=${profile_dir}"
    ]
  }
}
EOF
fi

(
  cd "${crate_root}"
  cp "${webdriver_config}" webdriver.json
  trap 'rm -f webdriver.json' EXIT
  wasm-bindgen-test-runner "${repo_root}/${wasm_artifact}"
)
