"""Create a bounded reading fixture in EMPTY, CLI-migrated test databases.

Never point this at a daily installation. Existing feeds/entries/content are
refused. The matching loopback RSS files are also generated for device refresh.
"""
import argparse
import base64
from datetime import datetime, timedelta, timezone
import hashlib
import json
from pathlib import Path
import sqlite3
import struct
from urllib.parse import urlparse
from xml.sax.saxutils import escape
import zlib


def png(index):
    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))
    row = bytes([60 + index * 30, 100, 160 - index * 20]) * 640
    raw = (b"\0" + row) * 360
    image = (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", 640, 360, 8, 2, 0, 0, 0))
             + chunk(b"IDAT", zlib.compress(raw)) + chunk(b"IEND", b""))
    return "data:image/png;base64," + base64.b64encode(image).decode()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("database_dir", type=Path)
    parser.add_argument("--fixture-dir", required=True, type=Path)
    parser.add_argument("--fixture-base", required=True)
    parser.add_argument("--entries", type=int, default=5000)
    parser.add_argument("--paragraphs", type=int, default=120)
    args = parser.parse_args()
    base = args.fixture_base.rstrip("/")
    parsed = urlparse(base)
    if parsed.scheme != "http" or parsed.hostname != "127.0.0.1" or parsed.path or parsed.query or parsed.fragment:
        parser.error("fixture-base must be a bare http://127.0.0.1:PORT origin")
    if not 100 <= args.entries <= 10000 or not 20 <= args.paragraphs <= 300:
        parser.error("entries must be 100..10000 and paragraphs 20..300")
    if args.fixture_dir.exists() and any(args.fixture_dir.iterdir()):
        parser.error("fixture-dir must be new or empty")
    index_path = args.database_dir / "rss-reader.db"
    content_path = args.database_dir / "rss-reader-content.db"
    if not index_path.is_file() or not content_path.is_file():
        parser.error("Databases must already be migrated by the matching CLI")
    fixture_root = Path(__file__).resolve().parents[1] / "tests/fixtures/browser_state"
    fixture = {name: json.loads((fixture_root / f"directory_contract_{name}.json").read_text(encoding="utf-8"))
               for name in ["core", "app_state", "entry_flags", "entry_content"]}
    feeds = fixture["core"]["feeds"]
    for feed in feeds:
        feed["title"] = ("用于原生阅读性能基线的超长来源名称 Native Reading Baseline Long Source Name "
                         if feed["id"] == 1 else "阅读测试来源 ") + str(feed["id"])
        feed["url"] = f"{base}/feed-{feed['id']}.xml"
    paragraphs = [f"<p>第 {i + 1} 段：这是一段用于阅读、选择复制和位置恢复验收的固定内容。"
                  "Reading baseline preserves readable text, links and scroll position while changing articles.</p>"
                  for i in range(args.paragraphs)]
    for i in range(4):
        paragraphs.insert(i * 25 + 1, f'<p><img src="{png(i)}" alt="本地测试图片 {i + 1}"></p>')
    long_body = "".join(paragraphs) + "<pre><code>fn main() { println!(\"RSS Reader\"); }</code></pre>"
    start = datetime(2026, 10, 25, 12, tzinfo=timezone.utc)
    xml_items = {feed["id"]: [] for feed in feeds}
    with sqlite3.connect(index_path) as index, sqlite3.connect(content_path) as content:
        index.execute("BEGIN IMMEDIATE")
        content.execute("BEGIN IMMEDIATE")
        for db, table in [(index, "feeds"), (index, "entries"), (content, "entry_contents")]:
            if db.execute(f"SELECT count(*) FROM {table}").fetchone()[0]:
                raise SystemExit(f"Refusing nonempty {table}; use a fresh isolated directory")

        def insert(db, table, row):
            row = {key: (value.replace(" ", "T", 1).replace(" +00:00:00", "Z")
                         if key.endswith("_at") and isinstance(value, str) else value)
                   for key, value in row.items()}
            db.execute(f"INSERT INTO {table} ({','.join(row)}) VALUES ({','.join('?' for _ in row)})", tuple(row.values()))

        for feed in feeds:
            insert(index, "feeds", feed)
        for ordinal in range(args.entries):
            entry_id = args.entries - ordinal
            feed_id = ordinal % len(feeds) + 1
            date = start - timedelta(minutes=ordinal * 10)
            stamp = date.isoformat().replace("+00:00", "Z")
            title = f"阅读基线文章 {entry_id:05d} — Reading baseline"
            body = long_body if ordinal < 24 else paragraphs[0]
            row = {**fixture["core"]["entries"][0], **fixture["entry_flags"]["entries"][0],
                   "id": entry_id, "feed_id": feed_id, "external_id": f"reading-{entry_id}",
                   "dedup_key": f"reading-{entry_id}", "url": f"{base}/entry/{entry_id}",
                   "title": title, "summary": "Deterministic isolated reading baseline.",
                   "is_read": False, "is_starred": False}
            for field in ["published_at", "first_seen_at", "created_at", "updated_at"]:
                row[field] = stamp
            insert(index, "entries", row)
            insert(content, "entry_contents", {
                **fixture["entry_content"]["entries"][0], "entry_id": entry_id, "feed_id": feed_id,
                "content_html": body, "content_text": "Reading baseline content.",
                "content_hash": hashlib.sha256(body.encode()).hexdigest(), "updated_at": stamp,
            })
            xml_items[feed_id].append(f"<item><guid>reading-{entry_id}</guid><title>{escape(title)}</title>"
                                     f"<link>{base}/entry/{entry_id}</link><pubDate>{date.strftime('%a, %d %b %Y %H:%M:%S GMT')}</pubDate>"
                                     f"<description>{escape(body)}</description></item>")
        for key, value in [("user_settings", fixture["core"]["settings"]), ("app_state_v2", fixture["app_state"])]:
            index.execute("INSERT OR REPLACE INTO app_settings (key,value,updated_at) VALUES (?,?,?)",
                          (key, json.dumps(value), start.isoformat()))
    args.fixture_dir.mkdir(parents=True, exist_ok=True)
    for feed in feeds:
        xml = f'<?xml version="1.0" encoding="utf-8"?><rss version="2.0"><channel><title>{escape(feed["title"])}</title><link>{base}</link><description>Reading baseline</description>'
        xml += "".join(xml_items[feed["id"]]) + "</channel></rss>"
        (args.fixture_dir / f"feed-{feed['id']}.xml").write_text(xml, encoding="utf-8")
    report = {"entries": args.entries, "feeds": len(feeds), "pageSize": 50, "longArticles": 24,
              "paragraphsPerLongArticle": args.paragraphs, "imagesPerLongArticle": 4,
              "imageDimensions": [640, 360], "fixtureBase": base, "schemaSource": "matching CLI migrations",
              "index": str(index_path.resolve()), "content": str(content_path.resolve())}
    (args.fixture_dir / "fixture.json").write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(report, ensure_ascii=False))


if __name__ == "__main__":
    main()
