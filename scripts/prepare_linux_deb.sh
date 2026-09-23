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
if [[ $(grep -c '^Version:' "$control") -ne 1 ]]; then
  echo "Debian control must contain exactly one Version field" >&2
  exit 1
fi
sed -i "s/^Version: .*/Version: $version/" "$control"
mkdir -p "$(dirname "$output")"
dpkg-deb --build --root-owner-group "$temp_dir/package" "$temp_dir/repacked.deb"
if [[ $(dpkg-deb -f "$temp_dir/repacked.deb" Version) != "$version" ]]; then
  echo "repacked Debian version does not match $tag" >&2
  exit 1
fi
mv -- "$temp_dir/repacked.deb" "$output"
