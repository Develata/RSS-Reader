#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
temp_dir=$(mktemp -d)
trap 'rm -rf -- "$temp_dir"' EXIT
mkdir -p "$temp_dir/input/DEBIAN" "$temp_dir/input/usr/bin"
cat > "$temp_dir/input/DEBIAN/control" <<'EOF'
Package: rssr-app
Version: 0.1.0
Architecture: amd64
Maintainer: RSS-Reader
Description: test package
EOF
cat > "$temp_dir/app.c" <<'EOF'
#include <stdio.h>
int main(void) { puts("RSS-Reader test"); return 0; }
EOF
cc -o "$temp_dir/input/usr/bin/rssr-app" "$temp_dir/app.c"
dpkg-deb --build --root-owner-group "$temp_dir/input" "$temp_dir/original.deb" >/dev/null

bash "$repo_root/scripts/prepare_linux_deb.sh" \
  "$temp_dir/original.deb" v0.1.17 "$temp_dir/output.deb" >/dev/null
test "$(dpkg-deb -f "$temp_dir/original.deb" Version)" = 0.1.0
test "$(dpkg-deb -f "$temp_dir/output.deb" Version)" = 0.1.17
dpkg-deb -f "$temp_dir/output.deb" Depends | grep -q 'libc6'
dpkg-deb -c "$temp_dir/output.deb" | grep -q 'root/root .*usr/bin/rssr-app$'
dpkg-deb -x "$temp_dir/output.deb" "$temp_dir/extracted"
cmp "$temp_dir/input/usr/bin/rssr-app" "$temp_dir/extracted/usr/bin/rssr-app"
if bash "$repo_root/scripts/prepare_linux_deb.sh" \
  "$temp_dir/original.deb" invalid "$temp_dir/rejected.deb" 2>/dev/null; then
  echo "invalid release tag was accepted" >&2
  exit 1
fi
test ! -e "$temp_dir/rejected.deb"

printf 'Depends: existing-runtime\n' >> "$temp_dir/input/DEBIAN/control"
dpkg-deb --build --root-owner-group "$temp_dir/input" "$temp_dir/existing-depends.deb" >/dev/null
if bash "$repo_root/scripts/prepare_linux_deb.sh" \
  "$temp_dir/existing-depends.deb" v0.1.17 "$temp_dir/rejected.deb" 2>/dev/null; then
  echo "existing Depends field was silently overwritten" >&2
  exit 1
fi
test ! -e "$temp_dir/rejected.deb"
