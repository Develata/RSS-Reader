#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 ]]; then
  echo "usage: $0 [--prepare DIR] <harness-name> [harness-name ...] | --prebuilt DIR <harness-name>" >&2
  exit 1
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# Preserve an explicitly selected/pinned runner before the fallback user tools.
export PATH="${PATH}:${HOME}/.local/bin:${HOME}/.cargo/bin"
cd "$repo_root"

# wasm_contract_runner.rs runs this runner: the explicit path if set, else the one in PATH.
runner="${RSSR_WASM_BINDGEN_TEST_RUNNER:-wasm-bindgen-test-runner}"
required_tools=(rustc)
if [[ "$1" != --prebuilt ]]; then required_tools+=(cargo); fi
if [[ "$1" != --prepare ]]; then required_tools+=("$runner" chromedriver); fi
for tool in "${required_tools[@]}"; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "$tool is required but was not found" >&2
    exit 1
  fi
done

# The runner must be the wasm-bindgen version Cargo.lock builds the tests with; another version
# fails late, or only as a missing path when a pinned copy has gone.
if [[ "$1" != --prepare ]]; then
  locked="$(awk '/^name = "wasm-bindgen"$/ { getline; gsub(/version = |"/, ""); print }' Cargo.lock)"
  if [[ -z "$locked" || "$locked" == *$'\n'* ]]; then
    echo "expected exactly one wasm-bindgen version in Cargo.lock, found: ${locked:-none}" >&2
    exit 1
  fi
  found="$("$runner" --version 2>/dev/null || true)"
  if [[ "$found" != "wasm-bindgen-test-runner $locked" ]]; then
    echo "$runner reports '${found:-no version}', but Cargo.lock locks wasm-bindgen $locked;" \
      "install wasm-bindgen-cli $locked or set RSSR_WASM_BINDGEN_TEST_RUNNER to it" >&2
    exit 1
  fi
fi

runner_dir="$(mktemp -d)"
trap 'rm -rf "$runner_dir"' EXIT
rustc --edition=2024 scripts/wasm_contract_runner.rs -o "$runner_dir/wasm-contract-runner"
"$runner_dir/wasm-contract-runner" "$@"
