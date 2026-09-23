#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 3 ]]; then
  echo "usage: $0 INPUT_DEB vX.Y.Z OUTPUT_DEB" >&2
  exit 2
fi

input=$1
tag=$2
output=$3
if [[ ! -f $input || $input == "$output" ]]; then
  echo "expected an existing input, a distinct output, and a vX.Y.Z release tag" >&2
  exit 2
fi
if [[ ! $tag =~ ^v([0-9]+\.[0-9]+\.[0-9]+)$ ]]; then
  echo "invalid release tag: $tag" >&2
  exit 2
fi
version=${BASH_REMATCH[1]}

temp_dir=$(mktemp -d)
trap 'rm -rf -- "$temp_dir"' EXIT
dpkg-deb -R "$input" "$temp_dir/package"
control=$temp_dir/package/DEBIAN/control
if [[ ! -f $temp_dir/package/usr/bin/rssr-app ]]; then
  echo "Debian package does not contain usr/bin/rssr-app" >&2
  exit 1
fi

# Dioxus currently emits no Depends field. Derive runtime dependencies from
# the linked ELF on the same Ubuntu runner that built it; do not guess package
# names from shared-library SONAMEs or from the runner's installed dev packages.
mkdir -p "$temp_dir/debian"
{
  printf 'Source: rssr-app\nMaintainer: Develata\n\n'
  cat "$control"
} > "$temp_dir/debian/control"
dependency_result=$(cd "$temp_dir" && dpkg-shlibdeps -O package/usr/bin/rssr-app)
if [[ $dependency_result != shlibs:Depends=* || $dependency_result == shlibs:Depends= ]]; then
  echo "dpkg-shlibdeps did not produce runtime dependencies" >&2
  exit 1
fi
dependencies=${dependency_result#shlibs:Depends=}
python3 - "$control" "$version" "$dependencies" <<'PY'
from pathlib import Path
import sys

path, version, dependencies = Path(sys.argv[1]), sys.argv[2], sys.argv[3]
lines = path.read_text().splitlines()
if sum(line.startswith('Version:') for line in lines) != 1:
    raise SystemExit('Debian control must contain exactly one Version field')
if any(line.startswith('Depends:') for line in lines):
    raise SystemExit('bundler now writes Depends; review dependency merge before release')
lines = [f'Version: {version}' if line.startswith('Version:') else line for line in lines]
path.write_text('\n'.join([*lines, f'Depends: {dependencies}']) + '\n')
PY
mkdir -p "$(dirname "$output")"
dpkg-deb --build --root-owner-group "$temp_dir/package" "$temp_dir/repacked.deb"
if [[ $(dpkg-deb -f "$temp_dir/repacked.deb" Version) != "$version" ]]; then
  echo "repacked Debian version does not match $tag" >&2
  exit 1
fi
mv -- "$temp_dir/repacked.deb" "$output"
