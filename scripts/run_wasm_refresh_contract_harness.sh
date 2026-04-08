#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
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

cargo test -p rssr-infra --target wasm32-unknown-unknown --test wasm_refresh_contract_harness --no-run

wasm_artifact="$(
  find target/wasm32-unknown-unknown/debug/deps \
    -maxdepth 1 \
    -name 'wasm_refresh_contract_harness-*.wasm' \
    -printf '%T@ %p\n' \
    | sort -nr \
    | head -n1 \
    | cut -d' ' -f2-
)"

if [[ -z "${wasm_artifact}" ]]; then
  echo "failed to locate wasm_refresh_contract_harness artifact" >&2
  exit 1
fi

wasm-bindgen-test-runner "${wasm_artifact}"
