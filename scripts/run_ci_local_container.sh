#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
image_name="${CI_LOCAL_IMAGE_NAME:-rss-reader-ci-local:latest}"
container_name="${CI_LOCAL_CONTAINER_NAME:-rssr-ci-local}"

usage() {
  cat <<EOF
usage: $0 [--rebuild] [--no-host-network] [--cmd '<shell command>']

options:
  --rebuild           Rebuild the local CI image before running
  --no-host-network   Use Docker bridge networking instead of --network host
  --cmd '<command>'   Run a one-shot shell command inside the container

examples:
  $0
  $0 --rebuild
  $0 --cmd 'cargo test -p rssr-web'
  $0 --cmd 'bash scripts/setup_chrome_for_testing.sh && bash scripts/run_wasm_refresh_contract_harness.sh'
EOF
}

rebuild=0
use_host_network=1
run_cmd=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --rebuild)
      rebuild=1
      shift
      ;;
    --no-host-network)
      use_host_network=0
      shift
      ;;
    --cmd)
      run_cmd="${2:-}"
      if [[ -z "$run_cmd" ]]; then
        echo "--cmd requires a value" >&2
        exit 1
      fi
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown option: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if [[ $rebuild -eq 1 ]] || ! docker image inspect "$image_name" >/dev/null 2>&1; then
  docker build -f "${repo_root}/Dockerfile.ci-local" -t "$image_name" "$repo_root"
fi

docker_args=(
  run
  --rm
  -it
  --name "$container_name"
  --workdir /work
  --mount "type=bind,src=${repo_root},target=/work"
  --mount "type=volume,src=rssr-cargo-registry,target=/usr/local/cargo/registry"
  --mount "type=volume,src=rssr-cargo-git,target=/usr/local/cargo/git"
  --mount "type=volume,src=rssr-target,target=/work/target"
  --shm-size=2g
)

if [[ $use_host_network -eq 1 ]]; then
  docker_args+=(--network host)
fi

if [[ -n "$run_cmd" ]]; then
  docker "${docker_args[@]}" "$image_name" bash -lc "$run_cmd"
else
  docker "${docker_args[@]}" "$image_name"
fi
