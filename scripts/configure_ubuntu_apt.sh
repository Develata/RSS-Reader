#!/usr/bin/env bash
set -euo pipefail

# The hosted runner's Azure mirror has stalled downloads. Keep Ubuntu suites,
# signatures and third-party sources unchanged, using the official main archive.
# mirror+file sources also choose their download host from apt-mirrors.txt.
for source in /etc/apt/sources.list /etc/apt/sources.list.d/*.list \
  /etc/apt/sources.list.d/*.sources /etc/apt/apt-mirrors.txt; do
  if [[ -f "$source" ]]; then
    sudo sed -i 's|https\?://azure\.archive\.ubuntu\.com/ubuntu|https://archive.ubuntu.com/ubuntu|g' "$source"
    printf 'Checked Ubuntu mirror selection in %s\n' "$source"
  fi
done
