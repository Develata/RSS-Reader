#!/usr/bin/env bash
# The Web page against the real server (v0.2 phase C, C4c): headless Chrome drives the page served
# by `rssr-web`, restarted on a fixture library per seed (scripts/browser/server_harness.mjs).
#
# Suites (--suite):
#   regression    small viewport UI regression, nav collapse, reading position, reader metadata and
#                 local time, and (default theme only) the directory contracts; --preset selects
#                 the theme
#   theme-matrix  the reader in every theme
#   feed-smoke    a first run: add a subscription through the form, refresh it, read its entry
#   acceptance    the browser acceptance of the server mode (scripts/browser/rssr_server_acceptance.mjs:
#                 reverse proxy, cookies, CSRF, unknown outcomes, jobs, rereads, drafts, build
#                 mismatch, reader body, start-up failures); --long adds the scenarios that wait
#                 30 s or more
#
# The page and the server must come from one build (the server refuses writes from another build;
# the harness checks the ids before the first check). Unless given, the bundle is built with dx
# and the server and the fixture tool with Cargo, from this checkout, with the caller's
# RSSR_BUILD_ID if set. --skip-build uses the bundle already built
# (target/dx/rssr-app/<profile>/web/public); Cargo still builds the server and the fixture tool
# (nothing to do when they are current) unless --server-bin / --fixture-bin name them.
set -euo pipefail

task_no_proxy="127.0.0.1,localhost,${no_proxy:-}"
export no_proxy="$task_no_proxy"
export NO_PROXY="$task_no_proxy"

suite="regression"
profile="debug"
port="8091"
skip_build="false"
log_dir=""
chrome_bin="${CHROME_BIN:-google-chrome}"
node_bin="${NODE_BIN:-node}"
viewport="360,800"
preset=""
directory_only="false"
directory_scroll_only="false"
long="false"
server_bin=""
fixture_bin=""
static_dir=""

usage() {
  echo "Usage: $0 [--suite regression|theme-matrix|feed-smoke|acceptance] [--port PORT] [--debug|--release] [--skip-build] [--log-dir DIR] [--chrome-bin BIN] [--viewport WIDTH,HEIGHT] [--preset PRESET] [--directory-only|--directory-scroll-only] [--long] [--server-bin BIN] [--fixture-bin BIN] [--static-dir DIR]" >&2
  exit 1
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --suite) suite="${2:?missing suite value}"; shift 2 ;;
    --port) port="${2:?missing port value}"; shift 2 ;;
    --debug) profile="debug"; shift ;;
    --release) profile="release"; shift ;;
    --skip-build) skip_build="true"; shift ;;
    --log-dir) log_dir="${2:?missing log dir value}"; shift 2 ;;
    --chrome-bin) chrome_bin="${2:?missing chrome bin value}"; shift 2 ;;
    --viewport) viewport="${2:?missing viewport value}"; shift 2 ;;
    --preset) preset="${2:?missing preset value}"; shift 2 ;;
    --directory-only) directory_only="true"; shift ;;
    --directory-scroll-only) directory_scroll_only="true"; shift ;;
    --long) long="true"; shift ;;
    --server-bin) server_bin="${2:?missing server bin value}"; shift 2 ;;
    --fixture-bin) fixture_bin="${2:?missing fixture bin value}"; shift 2 ;;
    --static-dir) static_dir="${2:?missing static dir value}"; shift 2 ;;
    *) usage ;;
  esac
done

case "$suite" in
  regression|theme-matrix|feed-smoke|acceptance) ;;
  *) echo "Unknown suite '${suite}'" >&2; usage ;;
esac
if [[ "$suite" != regression && ( -n "$preset" || "$directory_only" == true || "$directory_scroll_only" == true ) ]]; then
  echo "--preset and directory options apply to the regression suite only" >&2
  exit 1
fi
if [[ "$suite" != acceptance && "$long" == true ]]; then
  echo "--long applies to the acceptance suite only" >&2
  exit 1
fi
if [[ ! "$viewport" =~ ^([0-9]+),([0-9]+)$ ]]; then
  echo "Invalid viewport '${viewport}'; expected WIDTH,HEIGHT" >&2
  exit 1
fi
viewport_width="${BASH_REMATCH[1]}"
viewport_height="${BASH_REMATCH[2]}"
if [[ ! "$port" =~ ^[0-9]{1,5}$ ]] || ((10#$port < 1 || 10#$port > 55535)); then
  echo "Invalid port '${port}'; expected 1..55535 (CDP uses port + 10000)" >&2
  exit 1
fi
port="$((10#$port))"
cdp_port="$((port + 10000))"

resolve_chrome_bin() {
  if command -v "$chrome_bin" >/dev/null 2>&1; then
    command -v "$chrome_bin"
    return
  fi
  local candidates=(
    "/mnt/c/Program Files/Google/Chrome/Application/chrome.exe"
    "/mnt/c/Program Files (x86)/Google/Chrome/Application/chrome.exe"
    "/c/Program Files/Google/Chrome/Application/chrome.exe"
    "/c/Program Files (x86)/Google/Chrome/Application/chrome.exe"
    "${LOCALAPPDATA:-}/Google/Chrome/Application/chrome.exe"
  )
  local candidate
  for candidate in "${candidates[@]}"; do
    if [[ -n "$candidate" && -x "$candidate" ]]; then
      printf '%s\n' "$candidate"
      return
    fi
  done
  echo "Chrome executable not found; pass --chrome-bin or set CHROME_BIN" >&2
  exit 1
}

resolve_node_bin() {
  if command -v "$node_bin" >/dev/null 2>&1; then
    command -v "$node_bin"
    return
  fi
  if command -v node.exe >/dev/null 2>&1; then
    command -v node.exe
    return
  fi
  echo "Node.js executable not found; set NODE_BIN" >&2
  exit 1
}

chrome_bin="$(resolve_chrome_bin)"
node_bin="$(resolve_node_bin)"

if [[ -z "$log_dir" ]]; then
  log_dir="target/server-web-regression/${suite}/$(date +%Y%m%d-%H%M%S)"
fi
mkdir -p "$log_dir"
chrome_log="$log_dir/chrome.log"
build_log="$log_dir/build.log"
summary_file="$log_dir/summary.md"
chrome_profile="$log_dir/chrome-profile"
chrome_profile_arg="$chrome_profile"
if [[ "$chrome_bin" == *.exe ]]; then
  if command -v wslpath >/dev/null 2>&1; then
    chrome_profile_arg="$(wslpath -w "$(realpath -m "$chrome_profile")")"
  elif command -v cygpath >/dev/null 2>&1; then
    chrome_profile_arg="$(cygpath -w "$(realpath -m "$chrome_profile")")"
  fi
fi

# Probe both listeners before starting anything: an existing HTTP or CDP endpoint must never make a
# failed new instance look like a passing run.
"$node_bin" -e '
const net = require("node:net");
(async () => {
  for (const [label, port] of [["HTTP", process.argv[1]], ["CDP", process.argv[2]]]) {
    await new Promise((resolve) => {
      const probe = net.createServer();
      probe.once("error", (error) => { console.error(`${label} port ${port} is unavailable: ${error.message}`); process.exit(1); });
      probe.listen(Number(port), "127.0.0.1", () => probe.close(resolve));
    });
  }
})();' "$port" "$cdp_port"

cargo_profile_args=()
cargo_dir="debug"
dx_args=(build --platform web --package rssr-app --locked)
if [[ "$profile" == release ]]; then
  cargo_profile_args=(--release)
  cargo_dir="release"
  dx_args+=(--release)
fi
exe_suffix=""
case "$(uname -s)" in
  CYGWIN*|MINGW*|MSYS*) exe_suffix=".exe" ;;
esac

: >"$build_log"
if [[ -z "$static_dir" ]]; then
  static_dir="target/dx/rssr-app/${profile}/web/public"
  if [[ "$skip_build" != true ]]; then
    echo "Building the page (dx ${dx_args[*]})..."
    dx "${dx_args[@]}" >>"$build_log" 2>&1 || { echo "Page build failed; see ${build_log}" >&2; exit 1; }
  fi
fi
if [[ -z "$server_bin" ]]; then
  echo "Building rssr-web (${profile})..."
  cargo build --locked "${cargo_profile_args[@]}" -p rssr-web >>"$build_log" 2>&1 \
    || { echo "rssr-web build failed; see ${build_log}" >&2; exit 1; }
  server_bin="target/${cargo_dir}/rssr-web${exe_suffix}"
fi
if [[ -z "$fixture_bin" ]]; then
  echo "Building the fixture tool (${profile})..."
  cargo build --locked "${cargo_profile_args[@]}" -p rssr-infra --example browser_fixture_library >>"$build_log" 2>&1 \
    || { echo "Fixture tool build failed; see ${build_log}" >&2; exit 1; }
  fixture_bin="target/${cargo_dir}/examples/browser_fixture_library${exe_suffix}"
fi
for file in "$server_bin" "$fixture_bin" "$static_dir/index.html"; do
  if [[ ! -f "$file" ]]; then
    echo "Not found: ${file}" >&2
    exit 1
  fi
done

# Dioxus injects document::Title into the template at build time. Check the served bundle so a
# stale template title cannot appear twice before the client runtime mounts.
titles="$(grep -oi '<title>[^<]*</title>' "$static_dir/index.html" || true)"
if [[ "$titles" != "<title>RSS-Reader</title>" ]]; then
  echo "Unexpected initial Web title in ${static_dir}/index.html: ${titles}" >&2
  exit 1
fi

chrome_pid=""
stop_chrome() {
  if [[ -n "$chrome_pid" ]] && kill -0 "$chrome_pid" >/dev/null 2>&1; then
    kill "$chrome_pid" >/dev/null 2>&1 || true
    for _ in {1..30}; do
      kill -0 "$chrome_pid" >/dev/null 2>&1 || break
      sleep 0.1
    done
    kill -KILL "$chrome_pid" >/dev/null 2>&1 || true
  fi
  [[ -z "$chrome_pid" ]] || wait "$chrome_pid" >/dev/null 2>&1 || true
}
trap stop_chrome EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

"$chrome_bin" \
  --headless=new \
  --disable-gpu \
  --disable-background-networking \
  --no-first-run \
  --no-default-browser-check \
  --no-proxy-server \
  --no-sandbox \
  --remote-allow-origins='*' \
  --remote-debugging-port="$cdp_port" \
  --user-data-dir="$chrome_profile_arg" \
  about:blank >"$chrome_log" 2>&1 &
chrome_pid=$!

cdp_ready="false"
for _ in {1..30}; do
  if ! kill -0 "$chrome_pid" >/dev/null 2>&1; then
    echo "Chrome exited; see ${chrome_log}" >&2
    exit 1
  fi
  if curl --noproxy '*' --connect-timeout 2 --max-time 5 -fsS "http://127.0.0.1:${cdp_port}/json/version" >/dev/null 2>&1; then
    cdp_ready="true"
    break
  fi
  sleep 1
done
if [[ "$cdp_ready" != "true" ]]; then
  echo "Chrome CDP failed to become ready on port ${cdp_port}; see ${chrome_log}" >&2
  exit 1
fi

driver="scripts/browser/rssr_small_viewport_assertions.mjs"
driver_args=(
  --cdp-base "http://127.0.0.1:${cdp_port}"
  --server-bin "$server_bin"
  --fixture-bin "$fixture_bin"
  --static-dir "$static_dir"
  --artifact-dir "$log_dir"
  --port "$port"
)
if [[ "$suite" == acceptance ]]; then
  # Its own driver, at a desktop viewport of its own.
  driver="scripts/browser/rssr_server_acceptance.mjs"
  if [[ "$long" == true ]]; then driver_args+=(--long); fi
else
  driver_args+=(--suite "$suite" --width "$viewport_width" --height "$viewport_height" --dpr 3)
fi
if [[ "$suite" == regression ]]; then
  driver_args+=(--preset "$preset" --directory-only "$directory_only" --directory-scroll-only "$directory_scroll_only")
fi

case "$suite" in
  regression)
    artifacts="entries / feeds / settings / reader、entries-short-directory、entries-desktop、reader-desktop、reader-search、reader-image-viewer、entries-persistent-pagination、sources-full-names、nav-*、task1-* 的 HTML 与 PNG"
    ;;
  theme-matrix)
    artifacts="default、atlas-sidebar、newsprint、amethyst-glass、midnight-ledger 的 HTML 与 PNG"
    ;;
  feed-smoke)
    artifacts="browser-feed-smoke 的 HTML 与 PNG"
    ;;
  acceptance)
    artifacts="每个场景的结果与检查（assertions.json；--long：${long}）；失败场景的页面 HTML、PNG 与 API 请求记录（<场景>-traffic.txt）"
    ;;
esac

result="失败"
status=0
"$node_bin" "$driver" "${driver_args[@]}" || status=$?
if [[ "$status" == 0 ]]; then
  result="通过"
fi

# The acceptance sets its own viewport (a desktop window); the other suites use the one above.
viewport_line="${viewport_width}×${viewport_height} @ DPR 3"
if [[ "$suite" == acceptance ]]; then
  viewport_line="1280×800 @ DPR 1（验收自己的视口）"
fi

cat >"$summary_file" <<EOF
# Web 页面 × rssr-web 回归（${suite}）

- 日期：$(date '+%Y-%m-%d %H:%M:%S %z')
- commit：$(git rev-parse --short HEAD 2>/dev/null || echo unknown)
- profile：${profile}
- viewport：${viewport_line}
- preset：${preset:-default}
- directory-only：${directory_only}
- directory-scroll-only：${directory_scroll_only}
- 页面：${static_dir}
- 服务器：${server_bin}
- chrome：${chrome_bin}
- 结果：${result}

## 产物

- assertions.json（全部断言、浏览器控制台错误、被忽略的错误及原因）
- ${artifacts}
- server.log（rssr-web 与夹具准备）、feed-fixture.log（服务器的订阅请求）、chrome.log、build.log
EOF

if [[ "$status" != 0 ]]; then
  echo "server web ${suite} failed; see ${log_dir}/assertions.json" >&2
  exit "$status"
fi
echo "server web ${suite} passed"
echo "Summary: ${summary_file}"
echo "Artifacts: ${log_dir}"
