#!/usr/bin/env python3
"""Exercise a real legacy CLI -> candidate SQLite upgrade using synthetic data only.

Both executables must be supplied explicitly. The output directory must not exist;
no existing user database is accepted. The legacy executable creates schema and
ingests feeds through its public CLI. Only favorites (no CLI command) are seeded
with SQL. Original databases, command logs and every failed/retried copy survive.
"""

import argparse
import base64
import hashlib
import json
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
CSS = ":root { --release-upgrade-sentinel: v0121; }"


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


def snapshot(directory):
    result = {}
    for name, tables in (
        (DB_NAMES[0], ("feeds", "entries", "app_settings", "sync_sources")),
        (DB_NAMES[1], ("entry_contents",)),
    ):
        with sqlite3.connect(directory / name) as conn:
            assert conn.execute("PRAGMA integrity_check").fetchall() == [("ok",)]
            assert conn.execute("PRAGMA foreign_key_check").fetchall() == []
            conn.row_factory = sqlite3.Row
            for table in tables:
                rows = [dict(row) for row in conn.execute(f'SELECT * FROM "{table}" ORDER BY 1')]
                # 0006 adds a fence; compare all pre-existing columns byte-for-byte.
                if table == "feeds":
                    for row in rows:
                        row.pop("generation", None)
                result[table] = rows
    return result


def versions(directory):
    with sqlite3.connect(directory / DB_NAMES[0]) as conn:
        return conn.execute("SELECT version, success FROM _sqlx_migrations ORDER BY version").fetchall()


def checkpoint(directory):
    for name in DB_NAMES:
        with sqlite3.connect(directory / name) as conn:
            assert conn.execute("PRAGMA wal_checkpoint(TRUNCATE)").fetchone()[0] == 0


def copy_databases(source, destination):
    destination.mkdir(parents=True, exist_ok=False)
    for name in DB_NAMES:
        shutil.copy2(source / name, destination / name)
    return destination


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
            "Forced process termination covers migration lock wait, not power loss during disk flush.",
            "Does not certify GUI, Android hardware, signing, or other platform packages.",
        ],
    }
    commands = []

    def record(name):
        report["checks"].append({"name": name, "status": "passed"})
        print("PASS " + name, flush=True)

    def run(binary, directory, *command, expect_success=True):
        argv = [str(binary), "--database-url", database_url(directory), *command]
        result = subprocess.run(argv, capture_output=True, timeout=90)
        commands.append({"argv": argv, "exit_code": result.returncode,
                         "stdout": result.stdout.decode("utf-8", "replace"),
                         "stderr": result.stderr.decode("utf-8", "replace")})
        (root / "commands.json").write_text(json.dumps(commands, ensure_ascii=False, indent=2), encoding="utf-8")
        assert (result.returncode == 0) == expect_success, commands[-1]
        return result

    try:
        original = root / "legacy-original"
        original.mkdir()
        server = ThreadingHTTPServer(("127.0.0.1", 0), Fixture)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            run(legacy, original, "list-feeds")
            for name in ("alpha", "bravo", "charlie"):
                run(legacy, original, "add-feed", f"http://127.0.0.1:{server.server_port}/{name}.xml",
                    "--folder", "Upgrade fixture")
            run(legacy, original, "save-settings", "--theme", "dark", "--reader-font-scale", "1.2",
                "--refresh-interval-minutes", "1440", "--archive-after-months", "1200", "--custom-css", CSS)
            run(legacy, original, "mark-read", "--feed-id", "1", "--yes")
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=5)
        with sqlite3.connect(original / DB_NAMES[0]) as conn:
            conn.execute("UPDATE entries SET is_starred=1, starred_at=created_at WHERE id % 7 = 0")
        run(legacy, original, "show-settings")
        expected = snapshot(original)
        assert versions(original) == [(i, 1) for i in range(1, 6)]
        assert len(expected["feeds"]) == 3
        assert len(expected["entries"]) == len(expected["entry_contents"]) == 144
        assert sum(row["is_read"] for row in expected["entries"]) == 48
        assert sum(row["is_starred"] for row in expected["entries"]) == 20
        assert CSS in json.dumps(expected["app_settings"])
        checkpoint(original)
        original_hashes = {name: digest(original / name) for name in DB_NAMES}
        report["original_sha256"] = original_hashes
        (root / "legacy-snapshot.json").write_text(json.dumps(expected, ensure_ascii=False, indent=2), encoding="utf-8")
        record("legacy binary created migrations 1-5, 3 subscriptions, 144 bodies, 48 read, 20 starred, settings and CSS")

        def verify_upgraded(directory):
            assert versions(directory) == [(i, 1) for i in range(1, 8)]
            assert snapshot(directory) == expected
            with sqlite3.connect(directory / DB_NAMES[0]) as conn:
                assert conn.execute("SELECT DISTINCT generation FROM feeds").fetchall() == [(0,)]
                assert conn.execute("SELECT COUNT(*) FROM content_gc_queue").fetchone() == (0,)
                assert conn.execute("SELECT legacy_reconciled FROM content_gc_state WHERE id=1").fetchone() == (1,)

        upgraded = copy_databases(original, root / "upgrade-success")
        run(candidate, upgraded, "list-feeds")
        verify_upgraded(upgraded)
        record("0006 and 0007 upgrade preserve every pre-existing data column and body")
        for _ in range(3):
            result = run(candidate, upgraded, "show-settings")
            assert CSS in result.stdout.decode()
            verify_upgraded(upgraded)
        record("three independent candidate restarts preserve data and custom CSS")

        for failed_version in (6, 7):
            directory = copy_databases(original, root / f"fail-migration-{failed_version}")
            with sqlite3.connect(directory / DB_NAMES[0]) as conn:
                conn.execute(
                    "CREATE TRIGGER qa_fail_migration BEFORE INSERT ON _sqlx_migrations "
                    f"WHEN NEW.version={failed_version} BEGIN SELECT RAISE(ABORT, 'qa injected failure'); END"
                )
            result = run(candidate, directory, "list-feeds", expect_success=False)
            assert b"qa injected failure" in result.stderr
            assert versions(directory) == [(i, 1) for i in range(1, failed_version)]
            assert snapshot(directory) == expected
            with sqlite3.connect(directory / DB_NAMES[0]) as conn:
                if failed_version == 6:
                    assert "generation" not in [row[1] for row in conn.execute("PRAGMA table_info(feeds)")]
                else:
                    assert conn.execute("SELECT name FROM sqlite_master WHERE name LIKE 'content_gc_%'").fetchall() == []
                conn.execute("DROP TRIGGER qa_fail_migration")
            run(candidate, directory, "list-feeds")
            verify_upgraded(directory)
            record(f"migration {failed_version} injected failure rolls back DDL/history; retry succeeds")

        interrupted = copy_databases(original, root / "interrupted-lock-wait")
        with sqlite3.connect(interrupted / DB_NAMES[0]) as blocker:
            blocker.execute("BEGIN IMMEDIATE")
            argv = [str(candidate), "--database-url", database_url(interrupted), "list-feeds"]
            with (root / "interrupted-process.log").open("wb") as log:
                process = subprocess.Popen(argv, stdout=log, stderr=subprocess.STDOUT)
                try:
                    time.sleep(1)
                    assert process.poll() is None, "candidate did not wait for locked legacy database"
                    process.kill()
                    process.wait(timeout=10)
                finally:
                    if process.poll() is None:
                        process.kill()
                        process.wait(timeout=10)
            blocker.rollback()
        assert versions(interrupted) == [(i, 1) for i in range(1, 6)]
        assert snapshot(interrupted) == expected
        run(candidate, interrupted, "list-feeds")
        verify_upgraded(interrupted)
        record("forced termination while legacy database is write locked; restart upgrades safely")

        checkpoint(upgraded)
        downgrade = copy_databases(upgraded, root / "unsupported-downgrade-probe")
        result = run(legacy, downgrade, "list-feeds", expect_success=False)
        assert b"previously applied" in result.stderr and b"missing" in result.stderr
        assert snapshot(downgrade) == expected
        record("legacy binary refuses new migration history; old binary is not a rollback strategy")
        restored = copy_databases(original, root / "restore-original-to-legacy")
        run(legacy, restored, "list-feeds")
        assert snapshot(restored) == expected
        assert {name: digest(original / name) for name in DB_NAMES} == original_hashes
        record("restoring both original database files works with old binary; originals remain byte identical")
        report["status"] = "passed"
    except Exception as error:
        report["status"] = "failed"
        report["error"] = str(error)
        raise
    finally:
        (root / "summary.json").write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")


if __name__ == "__main__":
    main()
