#!/usr/bin/env python3
"""Exercise a real legacy CLI -> candidate v0.2 import using synthetic data only.

Both executables must be supplied explicitly. The output directory must not exist;
no existing user database is accepted. The legacy executable creates schema and
ingests feeds through its public CLI. Only favorites (no CLI command) are seeded
with SQL. The candidate imports copies of that library into a new `library.sqlite3`
through its own CLI (`--data-dir`). Original databases, command logs and every
failed/retried copy survive.
"""

import argparse
import base64
import hashlib
import json
import os
import shutil
import sqlite3
import struct
import subprocess
import threading
import time
import zlib
from datetime import datetime, timezone
from email.utils import format_datetime
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path


DB_NAMES = ("rss-reader.db", "rss-reader-content.db")
LIBRARY = "library.sqlite3"
CREATION_LOCK = LIBRARY + ".create-lock"
APPLICATION_ID = 0x52535352
CSS = ":root { --release-upgrade-sentinel: v0121; }"
# Released v0.1.x index migrations the importer accepts as a prefix (v0.1.21 has 1-5,
# v0.1.22 and v0.1.23 have 1-7).
RELEASED_MIGRATIONS = 7
TICKETS = ("refresh_issued", "refresh_published", "refresh_diagnosed")


def fixture_image():
    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))
    pixels = b"".join(b"\0" + bytes([40, 120 + row, 190]) * 96 for row in range(64))
    png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", 96, 64, 8, 2, 0, 0, 0))
    png += chunk(b"IDAT", zlib.compress(pixels)) + chunk(b"IEND", b"")
    return "data:image/png;base64," + base64.b64encode(png).decode()

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def database_url(directory):
    return "sqlite://" + (directory / DB_NAMES[0]).as_posix() + "?mode=rwc"


def rows(conn, sql):
    conn.row_factory = sqlite3.Row
    return [dict(row) for row in conn.execute(sql)]


def snapshot(directory):
    """Every row of every v0.1 table the import maps, the migration history and the
    AUTOINCREMENT high-water marks."""
    result = {}
    with sqlite3.connect(directory / DB_NAMES[0]) as conn:
        assert conn.execute("PRAGMA integrity_check").fetchall() == [("ok",)]
        assert conn.execute("PRAGMA foreign_key_check").fetchall() == []
        for table in ("feeds", "entries", "app_settings", "sync_sources"):
            result[table] = rows(conn, f'SELECT * FROM "{table}" ORDER BY 1')
        result["sqlite_sequence"] = rows(conn, "SELECT name, seq FROM sqlite_sequence ORDER BY name")
        result["migrations"] = [
            list(row) for row in conn.execute("SELECT version, success FROM _sqlx_migrations ORDER BY version")
        ]
    with sqlite3.connect(directory / DB_NAMES[1]) as conn:
        assert conn.execute("PRAGMA integrity_check").fetchall() == [("ok",)]
        result["entry_contents"] = rows(conn, "SELECT * FROM entry_contents ORDER BY entry_id")
    return result


def checkpoint(directory):
    for name in DB_NAMES:
        with sqlite3.connect(directory / name) as conn:
            assert conn.execute("PRAGMA wal_checkpoint(TRUNCATE)").fetchone()[0] == 0


def copy_databases(source, destination):
    destination.mkdir(parents=True, exist_ok=False)
    for name in DB_NAMES:
        shutil.copy2(source / name, destination / name)
    return destination


def scratch_files(directory):
    """Files next to the library other than the library, its WAL side files and its creation lock."""
    return sorted(
        path.name for path in directory.iterdir()
        if path.name.startswith(LIBRARY + ".") and path.name != CREATION_LOCK
    )


def verify_imported(directory, expected):
    """The library in `directory` holds every row of `expected` under the documented rules."""
    with sqlite3.connect(directory / LIBRARY) as conn:
        assert conn.execute("PRAGMA integrity_check").fetchall() == [("ok",)]
        assert conn.execute("PRAGMA foreign_key_check").fetchall() == []
        assert conn.execute("PRAGMA application_id").fetchone()[0] == APPLICATION_ID
        assert conn.execute("PRAGMA journal_mode").fetchone()[0] == "wal"
        feeds = rows(conn, "SELECT * FROM feeds ORDER BY id")
        entries = rows(conn, "SELECT * FROM entries ORDER BY id")
        bodies = rows(conn, "SELECT * FROM entry_contents ORDER BY entry_id")
        documents = {row["key"]: row for row in rows(conn, "SELECT * FROM app_settings")}
        sequence = {row["name"]: row["seq"] for row in rows(conn, "SELECT name, seq FROM sqlite_sequence")}

    # Feeds: every v0.1 column kept (generation included); names are source titles; tickets start at 0.
    assert len(feeds) == len(expected["feeds"])
    for old, new in zip(expected["feeds"], feeds):
        assert {column: new[column] for column in old} == old, (old, new)
        assert new["custom_title"] is None
        assert all(new[ticket] == 0 for ticket in TICKETS), new

    # Bodies: kept for live entries, without the redundant feed_id; has_content follows them.
    live = {entry["id"] for entry in entries}
    expected_bodies = [
        {column: value for column, value in body.items() if column != "feed_id"}
        for body in expected["entry_contents"] if body["entry_id"] in live
    ]
    assert bodies == expected_bodies
    with_body = {body["entry_id"] for body in bodies}

    # Entries: every v0.1 column kept except has_content (derived); title_search is the folded title
    # (the fixture titles are ASCII, where Python's str.lower() agrees with Rust's to_lowercase()).
    assert len(entries) == len(expected["entries"])
    for old, new in zip(expected["entries"], entries):
        kept = {column: value for column, value in old.items() if column != "has_content"}
        assert {column: new[column] for column in kept} == kept, (old, new)
        assert new["has_content"] == int(new["id"] in with_body), new
        assert new["title_search"] == new["title"].lower(), new

    # Documents: same JSON and time; nothing else.
    assert sorted(documents) == sorted(row["key"] for row in expected["app_settings"]), documents
    for old in expected["app_settings"]:
        new = documents[old["key"]]
        assert json.loads(new["value"]) == json.loads(old["value"]), (old, new)
        assert new["updated_at"] == old["updated_at"], (old, new)

    # AUTOINCREMENT marks: ids are never reused.
    for old in expected["sqlite_sequence"]:
        if old["name"] in ("feeds", "entries"):
            assert sequence[old["name"]] == old["seq"], (old, sequence)


class Fixture(BaseHTTPRequestHandler):
    def do_GET(self):
        title = self.path.strip("/").removesuffix(".xml").title()
        channel_title = title + (" - a deliberately long synthetic subscription name for native layout acceptance" if title == "Alpha" else "")
        items = []
        for number in range(48):
            month, day = 4 + number // 8, 1 + number % 8
            date = format_datetime(datetime(2026, month, day, 12, tzinfo=timezone.utc))
            body = ("<h2>Upgrade reading fixture</h2>"
                    f'<img src="{fixture_image()}" width="288" height="192" alt="Upgrade fixture image">') + "".join(
                f"<p>Paragraph {paragraph}: preserved {title} article {number}, synthetic content.</p>"
                for paragraph in range(60)
            )
            items.append(
                f"<item><guid isPermaLink='false'>{title}-{number}</guid>"
                f"<title>{title} article {number:02d}</title>"
                f"<link>https://example.invalid/{title}/{number}</link>"
                f"<pubDate>{date}</pubDate><description>Migration fixture</description>"
                f"<content:encoded><![CDATA[{body}]]></content:encoded></item>"
            )
        data = (
            '<?xml version="1.0"?><rss version="2.0" '
            'xmlns:content="http://purl.org/rss/1.0/modules/content/"><channel>'
            f"<title>{channel_title}</title><link>https://example.invalid/</link>"
            f"<description>Synthetic upgrade fixture</description>{''.join(items)}</channel></rss>"
        ).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/rss+xml")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def log_message(self, *_args):
        pass


def lock_file(handle):
    """Takes the exclusive lock the candidate takes on the creation lock file."""
    if os.name == "nt":
        import msvcrt
        msvcrt.locking(handle.fileno(), msvcrt.LK_NBLCK, 1)
    else:
        import fcntl
        fcntl.flock(handle.fileno(), fcntl.LOCK_EX)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--legacy-cli", type=Path, required=True)
    parser.add_argument("--candidate-cli", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    legacy, candidate = args.legacy_cli.resolve(strict=True), args.candidate_cli.resolve(strict=True)
    root = args.output_dir.resolve()
    root.mkdir(parents=True, exist_ok=False)
    report = {
        "legacy_cli": {"path": str(legacy), "sha256": digest(legacy)},
        "candidate_cli": {"path": str(candidate), "sha256": digest(candidate)},
        "checks": [],
        "limitations": [
            "Favorites seeded with SQL after legacy CLI ingestion (legacy CLI has no star command).",
            "Covers the split layout written by the legacy release; the inline v0.1.0-v0.1.6 layout "
            "is covered by fixture tests only.",
            "Forced process termination covers the creation-lock wait, not power loss during disk flush.",
            "Does not certify GUI, Android hardware, signing, or other platform packages.",
        ],
    }
    commands = []
    environment = {key: value for key, value in os.environ.items() if key != "RSSR_DATA_DIR"}

    def record(name):
        report["checks"].append({"name": name, "status": "passed"})
        print("PASS " + name, flush=True)

    def execute(argv, expect_success):
        result = subprocess.run(argv, capture_output=True, timeout=90, env=environment)
        commands.append({"argv": argv, "exit_code": result.returncode,
                         "stdout": result.stdout.decode("utf-8", "replace"),
                         "stderr": result.stderr.decode("utf-8", "replace")})
        (root / "commands.json").write_text(json.dumps(commands, ensure_ascii=False, indent=2), encoding="utf-8")
        assert (result.returncode == 0) == expect_success, commands[-1]
        return result

    def run_legacy(directory, *command, expect_success=True):
        return execute([str(legacy), "--database-url", database_url(directory), *command], expect_success)

    def candidate_argv(directory, *command):
        return [str(candidate), "--data-dir", str(directory), *command]

    def run_candidate(directory, *command, expect_success=True):
        return execute(candidate_argv(directory, *command), expect_success)

    try:
        original = root / "legacy-original"
        original.mkdir()
        server = ThreadingHTTPServer(("127.0.0.1", 0), Fixture)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            run_legacy(original, "list-feeds")
            for name in ("alpha", "bravo", "charlie"):
                run_legacy(original, "add-feed", f"http://127.0.0.1:{server.server_port}/{name}.xml",
                           "--folder", "Upgrade fixture")
            run_legacy(original, "save-settings", "--theme", "dark", "--reader-font-scale", "1.2",
                       "--refresh-interval-minutes", "1440", "--archive-after-months", "1200",
                       "--custom-css", CSS)
            run_legacy(original, "mark-read", "--feed-id", "1", "--yes")
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=5)
        with sqlite3.connect(original / DB_NAMES[0]) as conn:
            conn.execute("UPDATE entries SET is_starred=1, starred_at=created_at WHERE id % 7 = 0")
        run_legacy(original, "show-settings")
        checkpoint(original)
        expected = snapshot(original)
        legacy_last = len(expected["migrations"])
        assert expected["migrations"] == [[i, 1] for i in range(1, legacy_last + 1)], expected["migrations"]
        assert 5 <= legacy_last <= RELEASED_MIGRATIONS, expected["migrations"]
        report["legacy_migrations"] = legacy_last
        assert len(expected["feeds"]) == 3
        assert len(expected["entries"]) == len(expected["entry_contents"]) == 144
        assert sum(row["is_read"] for row in expected["entries"]) == 48
        assert sum(row["is_starred"] for row in expected["entries"]) == 20
        assert CSS in json.dumps(expected["app_settings"])
        assert expected["sync_sources"] == [], "sync_sources is not imported; the release never wrote it"
        original_hashes = {name: digest(original / name) for name in DB_NAMES}
        report["original_sha256"] = original_hashes
        (root / "legacy-snapshot.json").write_text(json.dumps(expected, ensure_ascii=False, indent=2), encoding="utf-8")
        record(f"legacy binary created migrations 1-{legacy_last}, 3 subscriptions, 144 bodies, 48 read, 20 starred, settings and CSS")

        imported = copy_databases(original, root / "import-success")
        run_candidate(imported, "list-feeds")
        verify_imported(imported, expected)
        assert scratch_files(imported) == [], scratch_files(imported)
        assert {name: digest(imported / name) for name in DB_NAMES} == original_hashes
        record("candidate imports every feed, entry, body, document and id mark; generation, times and "
               "flags kept; custom names empty; tickets 0; the v0.1 files are unchanged")

        for _ in range(3):
            result = run_candidate(imported, "show-settings")
            assert CSS in result.stdout.decode("utf-8", "replace")
            verify_imported(imported, expected)
        # The import runs once: later changes to the v0.1 files are not picked up.
        with sqlite3.connect(imported / DB_NAMES[0]) as conn:
            conn.execute("UPDATE feeds SET title = 'changed after import' WHERE id = 1")
        run_candidate(imported, "list-feeds")
        verify_imported(imported, expected)
        record("three independent candidate restarts preserve data and custom CSS; the import is not repeated")

        refused = copy_databases(original, root / "refuse-unknown-migration")
        with sqlite3.connect(refused / DB_NAMES[0]) as conn:
            conn.execute(
                "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time) "
                "VALUES (?, 'unknown', TRUE, x'00', 0)", (legacy_last + 1,)
            )
        result = run_candidate(refused, "list-feeds", expect_success=False)
        # Within the released range the checksum gives it away; past it, the version does.
        reason = "与发布版本不一致" if legacy_last < RELEASED_MIGRATIONS else "未发布的迁移"
        assert reason in result.stderr.decode("utf-8", "replace"), commands[-1]
        assert not (refused / LIBRARY).exists() and scratch_files(refused) == []
        with sqlite3.connect(refused / DB_NAMES[0]) as conn:
            conn.execute("DELETE FROM _sqlx_migrations WHERE version = ?", (legacy_last + 1,))
        run_candidate(refused, "list-feeds")
        verify_imported(refused, expected)
        record(f"an unknown migration {legacy_last + 1} is refused without creating a library; the fixed copy imports")

        stale = copy_databases(original, root / "stale-scratch-files")
        for name in ("importing", "import-index", "import-content", "importing-journal"):
            (stale / f"{LIBRARY}.{name}").write_bytes(b"left by a killed import")
        run_candidate(stale, "list-feeds")
        verify_imported(stale, expected)
        assert scratch_files(stale) == [], scratch_files(stale)
        record("scratch files left by a killed import are replaced; the import completes and cleans up")

        waiting = copy_databases(original, root / "interrupted-lock-wait")
        with (waiting / CREATION_LOCK).open("wb") as holder:
            lock_file(holder)
            with (root / "interrupted-process.log").open("wb") as log:
                process = subprocess.Popen(candidate_argv(waiting, "list-feeds"), stdout=log,
                                           stderr=subprocess.STDOUT, env=environment)
                try:
                    time.sleep(1)
                    assert process.poll() is None, "candidate did not wait for the creation lock"
                    process.kill()
                    process.wait(timeout=10)
                finally:
                    if process.poll() is None:
                        process.kill()
                        process.wait(timeout=10)
        assert not (waiting / LIBRARY).exists() and scratch_files(waiting) == []
        run_candidate(waiting, "list-feeds")
        verify_imported(waiting, expected)
        record("forced termination while another creator holds the lock leaves nothing; restart imports")

        run_legacy(imported, "list-feeds")
        restored = copy_databases(original, root / "restore-original-to-legacy")
        run_legacy(restored, "list-feeds")
        assert snapshot(restored) == expected
        assert {name: digest(original / name) for name in DB_NAMES} == original_hashes
        record("the legacy binary still opens its untouched files after an import; originals remain byte identical")
        report["status"] = "passed"
    except Exception as error:
        report["status"] = "failed"
        report["error"] = str(error)
        raise
    finally:
        (root / "summary.json").write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")


if __name__ == "__main__":
    main()
