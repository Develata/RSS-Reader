#!/usr/bin/env bash
set -euo pipefail

if [[ $(id -u) -eq 0 || ! -x /usr/bin/rssr-app ]]; then
  echo "run this smoke as an ordinary user after installing the .deb" >&2
  exit 2
fi

temp_dir=$(mktemp -d)
trap 'rm -rf -- "$temp_dir"' EXIT
export HOME="$temp_dir/普通 用户"
export XDG_DATA_HOME="$HOME/自定义 数据"
export XDG_RUNTIME_DIR="$temp_dir/runtime"
export GDK_BACKEND=x11
mkdir -p "$HOME" "$XDG_RUNTIME_DIR"
chmod 700 "$XDG_RUNTIME_DIR"
data_dir=$XDG_DATA_HOME/rss-reader

launch_once() {
  local exit_code=0
  xvfb-run -a timeout --kill-after=3s 10s /usr/bin/rssr-app \
    >"$temp_dir/app.log" 2>&1 || exit_code=$?
  if [[ $exit_code -ne 124 ]]; then
    cat "$temp_dir/app.log" >&2
    echo "installed app exited unexpectedly: $exit_code" >&2
    exit 1
  fi
}

launch_once
test -f "$data_dir/rss-reader.db"
test -f "$data_dir/rss-reader-content.db"
test "$(stat -c %a "$data_dir")" = 700
test ! -e /usr/bin/RSS-Reader
python3 - "$data_dir/rss-reader.db" <<'PY'
import sqlite3
import sys

with sqlite3.connect(sys.argv[1]) as db:
    db.execute('CREATE TABLE smoke_marker (value TEXT NOT NULL)')
    db.execute('INSERT INTO smoke_marker VALUES (?)', ('复用 数据',))
PY
launch_once
python3 - "$data_dir/rss-reader.db" <<'PY'
import sqlite3
import sys

with sqlite3.connect(sys.argv[1]) as db:
    assert db.execute('SELECT value FROM smoke_marker').fetchone() == ('复用 数据',)
PY
