"""Seed the directory fixture into EMPTY databases migrated by the matching CLI.

Copy rssr-app.exe and rssr-cli.exe into a new temporary directory, run that CLI's
list-feeds command, then pass its RSS-Reader subdirectory here. Never use a daily
installation. Existing feeds, entries or content cause refusal; nothing is deleted.
"""
import argparse
import json
import sqlite3
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("database_dir", type=Path)
    parser.add_argument("--fixture", choices=["directory_contract", "directory_scroll"], default="directory_contract")
    parser.add_argument("--fixture-base", required=True, help="Running loopback SPA fixture server, e.g. http://127.0.0.1:8114")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1] / "tests/fixtures/browser_state"
    fixture = {name: json.loads((root / f"{args.fixture}_{name}.json").read_text(encoding="utf-8"))
               for name in ["core", "app_state", "entry_flags", "entry_content"]}
    from urllib.parse import urlparse
    parsed = urlparse(args.fixture_base)
    if parsed.scheme != "http" or parsed.hostname not in ("127.0.0.1", "localhost"):
        parser.error("fixture-base must be a loopback HTTP server")
    for feed in fixture["core"]["feeds"]:
        feed["url"] = f"{args.fixture_base.rstrip('/')}/__codex/mobile-ui-feed.xml?seed=directory-contract&feed={feed['id']}"
    index_path = (args.database_dir / "rss-reader.db").resolve()
    content_path = (args.database_dir / "rss-reader-content.db").resolve()
    if not index_path.is_file() or not content_path.is_file():
        parser.error("Both databases must already exist and be migrated by the matching CLI")
    with sqlite3.connect(index_path) as index, sqlite3.connect(content_path) as content:
        index.execute("BEGIN IMMEDIATE")
        content.execute("BEGIN IMMEDIATE")
        for db, table in [(index, "feeds"), (index, "entries"), (content, "entry_contents")]:
            if db.execute(f"SELECT count(*) FROM {table}").fetchone()[0]:
                raise SystemExit(f"Refusing nonempty {table}; use a new isolated directory")

        def insert(db, table, rows):
            for row in rows:
                row = {key: (value.replace(" ", "T", 1).replace(" +00:00:00", "Z")
                             if key.endswith("_at") and isinstance(value, str) else value)
                       for key, value in row.items()}
                columns = ",".join(row)
                placeholders = ",".join("?" for _ in row)
                db.execute(f"INSERT INTO {table} ({columns}) VALUES ({placeholders})", tuple(row.values()))

        insert(index, "feeds", fixture["core"]["feeds"])
        flags = {row["id"]: row for row in fixture["entry_flags"]["entries"]}
        insert(index, "entries", [{**row, **flags[row["id"]]} for row in fixture["core"]["entries"]])
        insert(content, "entry_contents", fixture["entry_content"]["entries"])
        for key, value in [("user_settings", fixture["core"]["settings"]), ("app_state_v2", fixture["app_state"])]:
            index.execute("INSERT OR REPLACE INTO app_settings (key,value,updated_at) VALUES (?,?,?)",
                          (key, json.dumps(value), "2026-10-02T00:00:00Z"))
    print(json.dumps({"index": str(index_path), "content": str(content_path),
                      "feeds": len(fixture["core"]["feeds"]), "entries": len(fixture["core"]["entries"])}))


if __name__ == "__main__":
    main()
