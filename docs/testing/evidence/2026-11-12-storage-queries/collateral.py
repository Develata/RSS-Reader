# Plans of the other entries statements (copied from store/*.rs), current schema vs. the same
# library with the 0003 index (named cand_w); changed plans are timed. Write cost is the
# statements' own execution time inside transactions that are rolled back: it leaves out commit,
# fsync and WAL checkpoint cost. Usage: collateral.py <size> (reads base<size>.db, w<size>.db).
import os, sqlite3, sys, time
size = sys.argv[1]
def open_(p):
    c = sqlite3.connect(p, isolation_level=None)
    c.create_function("listed_title", 1, lambda s: None if s is None else s[:200], deterministic=True)
    c.create_function("fold_for_search", 1, lambda s: None if s is None else s.lower(), deterministic=True)
    return c
base, w = open_(f"base{size}.db"), open_(f"w{size}.db")
ACT = " FROM entries JOIN feeds ON feeds.id = entries.feed_id WHERE feeds.is_deleted = 0"
SK = "COALESCE(entries.published_at, entries.created_at)"
sort_at, cur = base.execute(f"SELECT {SK}, id FROM entries WHERE id = 12345").fetchone()
def adj(feed, unread, newer):
    cmp, order = (">", "ASC") if newer else ("<", "DESC")
    s = "SELECT entries.id" + ACT + (" AND entries.feed_id = ?" if feed else "") + (" AND entries.is_read = 0" if unread else "")
    s += f" AND ({SK} {cmp} ? OR ({SK} = ? AND entries.id {cmp} ?)) ORDER BY {SK} {order}, entries.id {order} LIMIT 1"
    return s, ([17] if feed else []) + [sort_at, sort_at, cur]
NOT_EXCL = " AND NOT (entries.is_archived = 0 AND (entries.published_at IS NULL OR entries.published_at >= ?))"
EXCL = " AND entries.is_archived = 0 AND (entries.published_at IS NULL OR entries.published_at >= ?)"
CUT = "2026-08-12T00:00:00Z"
Q = {
  "nav current": ("SELECT entries.feed_id, COALESCE(entries.published_at, entries.created_at) FROM entries JOIN feeds ON feeds.id = entries.feed_id WHERE entries.id = ?1 AND feeds.is_deleted = 0", [cur]),
  "nav prev unread": adj(None, True, True), "nav next unread": adj(None, True, False),
  "nav prev feed": adj(17, False, True), "nav next feed": adj(17, False, False),
  "unread_selection all": ("SELECT entries.id" + ACT + " AND entries.is_read = 0 ORDER BY entries.id LIMIT ?", [10001]),
  "unread_selection feed": ("SELECT entries.id" + ACT + " AND entries.is_read = 0 AND entries.feed_id = ? ORDER BY entries.id LIMIT ?", [17, 10001]),
  "unread_selection excl_auto": ("SELECT entries.id" + ACT + " AND entries.is_read = 0" + EXCL + " ORDER BY entries.id LIMIT ?", [CUT, 10001]),
  "unread_selection only_auto(not)": ("SELECT entries.id" + ACT + " AND entries.is_read = 0" + NOT_EXCL + " ORDER BY entries.id LIMIT ?", [CUT, 10001]),
  "mark_read UPDATE feed": ("UPDATE entries SET is_read = 1, read_at = 'n', updated_at = 'n' WHERE id IN (SELECT entries.id" + ACT + " AND entries.is_read = 0 AND entries.feed_id = ?)", [17]),
  "mark_read UPDATE excl_auto": ("UPDATE entries SET is_read = 1, read_at = 'n', updated_at = 'n' WHERE id IN (SELECT entries.id" + ACT + " AND entries.is_read = 0" + EXCL + ")", [CUT]),
  "batch mark read": ("UPDATE entries SET is_read = 1, read_at = ?1, updated_at = ?1 WHERE is_read = 0 AND id IN (SELECT value FROM json_each(?2)) AND feed_id IN (SELECT id FROM feeds WHERE is_deleted = 0)", ["n", "[1,2,3,4,5]"]),
  "batch archive": ("UPDATE entries SET is_archived = 1, updated_at = ?1 WHERE is_archived = 0 AND id IN (SELECT value FROM json_each(?2)) AND feed_id IN (SELECT id FROM feeds WHERE is_deleted = 0)", ["n", "[1,2,3,4,5]"]),
  "has_entries": ("SELECT EXISTS(SELECT 1 FROM entries WHERE feed_id = ?1)", [17]),
  "entry_count(feed)": ("SELECT COUNT(*) FROM entries WHERE feed_id = ?1", [17]),
  "promote exists": ("SELECT EXISTS(SELECT 1 FROM entries WHERE feed_id = ?1 AND (external_id = ?2 OR dedup_key = ?2))", [17, "x"]),
  "promote update": ("UPDATE entries SET external_id = ?2, dedup_key = ?2 WHERE id = (SELECT id FROM entries WHERE feed_id = ?1 AND external_id = ?3 AND dedup_key = ?3 AND url = ?3 LIMIT 1)", [17, "a", "u"]),
  "active_entry_generation": ("SELECT entries.feed_id, feeds.generation FROM entries JOIN feeds ON feeds.id = entries.feed_id WHERE entries.id = ?1 AND feeds.is_deleted = 0", [cur]),
  "body_for_localization": ("SELECT c.content_html, c.content_hash FROM entries e JOIN feeds f ON f.id = e.feed_id JOIN entry_contents c ON c.entry_id = e.id WHERE e.feed_id = ?1 AND e.dedup_key = ?2 AND f.generation = ?3 AND f.is_deleted = 0 AND length(CAST(c.content_html AS BLOB)) <= ?4", [17, "dedup-1", 0, 100000]),
  "delete feed entries": ("DELETE FROM entries WHERE feed_id = ?1", [17]),
}
def plan(c, s, p): return " | ".join(r[3] for r in c.execute("EXPLAIN QUERY PLAN " + s, p))
def timed(c, s, p):
    best = 1e9
    for _ in range(5):
        c.execute("BEGIN"); t = time.perf_counter(); c.execute(s, p).fetchall(); best = min(best, time.perf_counter() - t); c.execute("ROLLBACK")
    return best * 1000
for name, (s, p) in Q.items():
    pb, pw = plan(base, s, p), plan(w, s, p)
    if pb != pw:
        print(f"CHANGED {name}: {timed(base, s, p):.2f} -> {timed(w, s, p):.2f} ms\n   old {pb}\n   new {pw}")
    else:
        print(f"same    {name}: {pb}")
# Write cost: refresh-style upsert of 100 existing + 20 new rows for one feed, and a 2000-row mark-read.
ids = [r[0] for r in base.execute("SELECT dedup_key FROM entries WHERE feed_id = 17 LIMIT 100")]
UPSERT = """INSERT INTO entries (feed_id, external_id, dedup_key, url, title, title_search, author, summary, published_at, updated_at_source, first_seen_at, has_content, created_at, updated_at)
 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?11, ?11) ON CONFLICT(feed_id, dedup_key) DO UPDATE SET external_id = excluded.external_id, url = COALESCE(excluded.url, entries.url),
 title = excluded.title, title_search = excluded.title_search, author = excluded.author, summary = excluded.summary, published_at = COALESCE(excluded.published_at, entries.published_at),
 updated_at_source = COALESCE(excluded.updated_at_source, entries.updated_at_source), has_content = MAX(entries.has_content, excluded.has_content), updated_at = excluded.updated_at RETURNING id"""
rows = [(17, k, k, "https://u", "T", "t", None, "s" * 400, "2026-11-01T00:00:00Z", None, "2026-11-12T00:00:00Z", 1) for k in ids] + \
       [(17, f"new-{i}", f"new-{i}", "https://u", "T", "t", None, "s" * 400, "2026-11-01T00:00:00Z", None, "2026-11-12T00:00:00Z", 1) for i in range(20)]
for label, c in (("base", base), ("w", w)):
    best_u = best_m = 1e9
    for _ in range(7):
        c.execute("BEGIN"); t = time.perf_counter()
        for r in rows: c.execute(UPSERT, r).fetchall()
        best_u = min(best_u, time.perf_counter() - t); c.execute("ROLLBACK")
        c.execute("BEGIN"); t = time.perf_counter()
        c.execute("UPDATE entries SET is_read = 1, read_at = 'n', updated_at = 'n' WHERE id IN (SELECT id FROM entries WHERE is_read = 0 LIMIT 2000)")
        best_m = min(best_m, time.perf_counter() - t); c.execute("ROLLBACK")
    print(f"write {label}: upsert 120 rows {best_u*1000:.2f} ms, mark 2000 read {best_m*1000:.2f} ms")
t = time.perf_counter(); base.execute("BEGIN"); base.execute("CREATE INDEX tmp_w ON entries(feed_id, is_read, is_starred, is_archived, published_at)"); print(f"CREATE INDEX at {size}: {(time.perf_counter()-t)*1000:.0f} ms"); base.execute("ROLLBACK")
print("file size:", os.path.getsize(f"base{size}.db"), "bytes;", "with index:", os.path.getsize(f"w{size}.db"), "bytes")
