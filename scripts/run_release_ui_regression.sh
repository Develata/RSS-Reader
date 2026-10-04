#!/usr/bin/env bash
# Compatibility launcher only. Rust owns options, plan, state and exit codes.
set -euo pipefail
script_path="${BASH_SOURCE[0]}"
case "$OSTYPE" in
  msys*|cygwin*)
    # PowerShell may omit Git Bash tools; never move them ahead of caller tools.
    export PATH="$PATH:/usr/bin:/bin"
    export RSSR_BASH="${RSSR_BASH:-$(/usr/bin/cygpath -w "$BASH")}"
    script_path="${script_path//\\//}"
    ;;
  *) export RSSR_BASH="${RSSR_BASH:-$BASH}" ;;
esac
script_dir="${script_path%/*}"
if [[ "$script_dir" == "$script_path" ]]; then script_dir=.; fi
repo_root="$(cd "$script_dir/.." && pwd)"
cd "$repo_root"
# This is a host tool even when the caller configures a product cross target.
# Cargo resolves its configured compiler; bare rustc on PATH may be unrelated.
python=""
for candidate in python3 python; do
  if command -v "$candidate" >/dev/null 2>&1 &&
     "$candidate" -c 'import sys; sys.exit(sys.version_info.major != 3)' >/dev/null 2>&1; then
    python="$candidate"
    break
  fi
done
if [[ -z "$python" ]]; then
  echo 'Python 3 is required to read the Cargo launcher artifact (python3 or python on PATH)' >&2
  exit 1
fi
binary="$(cargo build --quiet --locked --manifest-path scripts/release-ui/Cargo.toml \
  --bin release-ui --target host-tuple --target-dir target/release-ui-runner \
  --message-format=json-render-diagnostics | "$python" -X utf8 -c '
import json
from pathlib import Path
import sys

executables = set()
for line in sys.stdin:
    try:
        message = json.loads(line)
    except json.JSONDecodeError:
        print(line, end="", file=sys.stderr)
        continue
    if (message.get("reason") == "compiler-artifact"
            and message.get("target", {}).get("name") == "release-ui"
            and "bin" in message.get("target", {}).get("kind", [])
            and not message.get("profile", {}).get("test")
            and message.get("executable")):
        executables.add(message["executable"])
if len(executables) != 1:
    sys.exit("Could not resolve a unique release-ui executable from Cargo")
print(Path(executables.pop()).as_posix())
')"
exec "$binary" "$@"
