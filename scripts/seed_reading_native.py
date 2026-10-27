"""Create a bounded reading fixture in a NEW single-library data directory.

Never point this at a daily installation. Existing data-directory contents are
refused. The matching Rust fixture tool owns the current schema. The matching loopback RSS files are also generated for device refresh.
"""
import argparse
import base64
from datetime import datetime, timedelta, timezone
import hashlib
import json
from pathlib import Path
import os
import subprocess
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
    suffix = ".exe" if os.name == "nt" else ""
    target = Path(os.environ.get("CARGO_TARGET_DIR", "target"))
    parser.add_argument("--fixture-bin", type=Path, default=target / "debug/examples" / ("browser_fixture_library" + suffix))
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
    if args.database_dir.exists() and any(args.database_dir.iterdir()):
        parser.error("database_dir must be new or empty")
    if not args.fixture_bin.is_file():
        parser.error("Build the matching browser_fixture_library example, or pass --fixture-bin")
    fixture_root = Path(__file__).resolve().parents[1] / "tests/fixtures/browser_state"
    fixture = {name: json.loads((fixture_root / f"directory_contract_{name}.json").read_text(encoding="utf-8"))
               for name in ["core", "app_state", "entry_flags", "entry_content"]}
    feeds = fixture["core"]["feeds"]
    for feed in feeds:
        feed["title"] = ("用于原生阅读性能基线的超长来源名称 Native Reading Baseline Long Source Name "
                         if feed["id"] == 1 else "阅读测试来源 ") + str(feed["id"])
        feed["url"] = f"{base}/reading-baseline/{feed['id']}.xml"
    paragraphs = [f"<p>第 {i + 1} 段：这是一段用于阅读、选择复制和位置恢复验收的固定内容。"
                  "Reading baseline preserves readable text, links and scroll position while changing articles.</p>"
                  for i in range(args.paragraphs)]
    for i in range(4):
        paragraphs.insert(i * 25 + 1, f'<p><img src="{png(i)}" alt="本地测试图片 {i + 1}"></p>')
    long_body = "".join(paragraphs) + "<pre><code>fn main() { println!(\"RSS Reader\"); }</code></pre>"
    start = datetime(2026, 10, 25, 12, tzinfo=timezone.utc)
    xml_items = {feed["id"]: [] for feed in feeds}
    fixture["core"]["entries"] = []
    fixture["core"]["next_entry_id"] = args.entries + 1
    fixture["entry_flags"]["entries"] = []
    fixture["entry_content"]["entries"] = []
    template = json.loads((fixture_root / "directory_contract_core.json").read_text(encoding="utf-8"))["entries"][0]
    for ordinal in range(args.entries):
        entry_id = args.entries - ordinal
        feed_id = ordinal % len(feeds) + 1
        date = start - timedelta(minutes=ordinal * 10)
        # The shared seed codec uses time's human-readable serde format.
        stamp = date.strftime("%Y-%m-%d %H:%M:%S.0 +00:00:00")
        title = f"阅读基线文章 {entry_id:05d} — Reading baseline"
        body = long_body if ordinal < 24 else paragraphs[0]
        row = {**template,
               "id": entry_id, "feed_id": feed_id, "external_id": f"reading-{entry_id}",
               "dedup_key": f"reading-{entry_id}", "url": f"{base}/entry/{entry_id}",
               "title": title, "summary": "Deterministic isolated reading baseline.",
               "is_read": False, "is_starred": False}
        for field in ["published_at", "first_seen_at", "created_at", "updated_at"]:
            row[field] = stamp
        fixture["core"]["entries"].append(row)
        fixture["entry_flags"]["entries"].append({"id":entry_id,"feed_id":feed_id,"is_read":False,"is_starred":False,"read_at":None,"starred_at":None})
        fixture["entry_content"]["entries"].append({ "entry_id": entry_id, "feed_id": feed_id,
            "content_html": body, "content_text": "Reading baseline content.",
            "content_hash": hashlib.sha256(body.encode()).hexdigest(), "updated_at": stamp,
        })
        xml_items[feed_id].append(f"<item><guid>reading-{entry_id}</guid><title>{escape(title)}</title>"
                                 f"<link>{base}/entry/{entry_id}</link><pubDate>{date.strftime('%a, %d %b %Y %H:%M:%S GMT')}</pubDate>"
                                 f"<description>{escape(body)}</description></item>")
    args.fixture_dir.mkdir(parents=True, exist_ok=True)
    seeds = args.fixture_dir / "seeds"
    seeds.mkdir()
    for name, value in fixture.items():
        (seeds / f"reading_baseline_{name}.json").write_text(json.dumps(value, ensure_ascii=False), encoding="utf-8")
    subprocess.run([str(args.fixture_bin.resolve()), "--data-dir", str(args.database_dir),
                    "--seeds", str(seeds), "--seed", "reading-baseline", "--feed-base", base], check=True)
    feed_dir = args.fixture_dir / "reading-baseline"
    feed_dir.mkdir()
    for feed in feeds:
        xml = f'<?xml version="1.0" encoding="utf-8"?><rss version="2.0"><channel><title>{escape(feed["title"])}</title><link>{base}</link><description>Reading baseline</description>'
        xml += "".join(xml_items[feed["id"]]) + "</channel></rss>"
        (feed_dir / f"{feed['id']}.xml").write_text(xml, encoding="utf-8")
    report = {"entries": args.entries, "feeds": len(feeds), "pageSize": 50, "longArticles": 24,
              "paragraphsPerLongArticle": args.paragraphs, "imagesPerLongArticle": 4,
              "imageDimensions": [640, 360], "fixtureBase": base, "schemaSource": "matching Rust browser_fixture_library",
              "library": str((args.database_dir / "library.sqlite3").resolve())}
    (args.fixture_dir / "fixture.json").write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(report, ensure_ascii=False))


if __name__ == "__main__":
    main()
