#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 ]]; then
  echo "usage: $0 <harness-name> [harness-name ...]" >&2
  exit 1
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# Preserve an explicitly selected/pinned runner before the fallback user tools.
export PATH="${PATH}:${HOME}/.local/bin:${HOME}/.cargo/bin"
cd "$repo_root"

for tool in rustc cargo wasm-bindgen-test-runner chromedriver; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "$tool is required in PATH" >&2
    exit 1
  fi
done

runner_dir="$(mktemp -d)"
trap 'rm -rf "$runner_dir"' EXIT
rustc --edition=2024 scripts/wasm_contract_runner.rs -o "$runner_dir/wasm-contract-runner"
"$runner_dir/wasm-contract-runner" "$@"
