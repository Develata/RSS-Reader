# Current main SQL on the current schema vs. the change (index 0003, lists over every subscription
# from entries, counts from feeds, OnlyArchived as a negation). Every read x starred x archive x
# scope x search shape for lists (LIMIT 2001) and counts, plus feed summaries; results must be
# identical, timings are interleaved best-of-N. Usage: final.py <base.db> <with-index.db> <N>
import sqlite3, sys, time, itertools, json, statistics
base_db, new_db, N = sys.argv[1], sys.argv[2], int(sys.argv[3])
def connect(path):
    c = sqlite3.connect(f"file:{path}?mode=ro", uri=True)
    c.create_function("listed_title", 1, lambda s: None if s is None else s[:200], deterministic=True)
    return c
cb, cn = connect(base_db), connect(new_db)
assert cn.execute("SELECT count(*) FROM sqlite_master WHERE name = 'cand_w'").fetchone()[0] == 1
S = "COALESCE(entries.published_at, entries.created_at)"
CUT = "2026-08-12T00:00:00Z"
JOIN = " FROM entries JOIN feeds ON feeds.id = entries.feed_id WHERE feeds.is_deleted = 0"
LIST_ALL = " FROM entries CROSS JOIN feeds ON feeds.id = entries.feed_id WHERE feeds.is_deleted = 0"
COUNT = " FROM feeds CROSS JOIN entries ON entries.feed_id = feeds.id WHERE feeds.is_deleted = 0"
FEEDS = json.dumps(list(range(3, 200, 7))[:30])
def filters(read, star, arch, scope, search, new):
    sql, p = "", []
    if scope == "feed": sql += " AND entries.feed_id = ?"; p.append(17)
    if scope == "feeds": sql += " AND entries.feed_id IN (SELECT value FROM json_each(?))"; p.append(FEEDS)
    sql += {"all": "", "unread": " AND entries.is_read = 0", "read": " AND entries.is_read = 1"}[read]
    sql += {"all": "", "starred": " AND entries.is_starred = 1", "unstarred": " AND entries.is_starred = 0"}[star]
    visible = "entries.is_archived = 0 AND (entries.published_at IS NULL OR entries.published_at >= ?)"
    if arch == "excl_manual": sql += " AND entries.is_archived = 0"
    if arch == "only_manual": sql += " AND entries.is_archived = 1"
    if arch == "excl_auto": sql += " AND " + visible; p.append(CUT)
    if arch == "only_auto":
        sql += (" AND NOT (" + visible + ")") if new else " AND (entries.is_archived = 1 OR (entries.published_at IS NOT NULL AND entries.published_at < ?))"
        p.append(CUT)
    if search: sql += " AND instr(entries.title_search, ?) > 0"; p.append("topic 42")
    return sql, p
HEAD = "SELECT entries.id, entries.feed_id, listed_title(entries.title), listed_title(COALESCE(feeds.custom_title, feeds.title, feeds.url)), entries.published_at, entries.is_read, entries.is_starred"
def list_sql(k, new):
    f, p = filters(*k, new); frm = (LIST_ALL if new and k[3] == "all" else JOIN)
    return HEAD + frm + f + f" ORDER BY {S} DESC, entries.id DESC LIMIT ?", p + [2001]
def count_sql(k, new):
    f, p = filters(*k, new); return "SELECT COUNT(*)" + (COUNT if new else JOIN) + f, p
SUMMARY = """SELECT feeds.id, listed_title(COALESCE(feeds.custom_title, feeds.title, feeds.url)), feeds.url,
 COALESCE(SUM(CASE WHEN entries.is_read = 0 THEN 1 ELSE 0 END), 0), COUNT(entries.id), feeds.last_fetched_at,
 feeds.last_success_at, feeds.fetch_error, feeds.fetch_error_kind FROM feeds LEFT JOIN entries ON entries.feed_id = feeds.id
 WHERE feeds.is_deleted = 0 GROUP BY feeds.id ORDER BY COALESCE(feeds.custom_title, feeds.title, feeds.url) ASC"""
combos = list(itertools.product(["all", "unread", "read"], ["all", "starred", "unstarred"],
    ["all", "excl_manual", "only_manual", "excl_auto", "only_auto"], ["all", "feed", "feeds"], [False, True]))
def run(c, sql, p):
    t = time.perf_counter(); r = c.execute(sql, p).fetchall(); return (time.perf_counter() - t) * 1000, r
def compare(build):
    out = {}
    for k in combos:
        sb, pb = build(k, False); sn, pn = build(k, True)
        bb = bn = 1e9
        for _ in range(N):
            t, rb = run(cb, sb, pb); bb = min(bb, t)
            t, rn = run(cn, sn, pn); bn = min(bn, t)
        assert rb == rn, k
        out[k] = (bb, bn)
    return out
report = {}
for name, build in (("list", list_sql), ("count", count_sql)):
    r = compare(build)
    deltas = sorted(((n - b), k, b, n) for k, (b, n) in r.items())
    report[name] = r
    print(f"[{name}] total {sum(b for b, _ in r.values()):.0f} -> {sum(n for _, n in r.values()):.0f} ms over {len(r)} shapes; "
          f"median {statistics.median(b for b, _ in r.values()):.1f} -> {statistics.median(n for _, n in r.values()):.1f} ms; "
          f">1 ms slower: {sum(d > 1 for d, *_ in deltas)}, >1 ms faster: {sum(d < -1 for d, *_ in deltas)}")
    for d, k, b, n in deltas[-3:]: print(f"   worst {d:+.1f} ms {k}: {b:.1f} -> {n:.1f}")
for k in [("all", "all", "excl_auto", "all", False), ("all", "all", "all", "all", False), ("unread", "all", "excl_auto", "all", False)]:
    b, n = report["list"][k]; print(f"list {k}: {b:.1f} -> {n:.1f} ms")
for k in [("all", "all", "only_auto", "all", False), ("all", "all", "only_manual", "all", False), ("unread", "all", "only_auto", "all", False), ("read", "all", "only_auto", "all", False)]:
    b, n = report["count"][k]; print(f"count {k}: {b:.1f} -> {n:.1f} ms")
sb = min(run(cb, SUMMARY, [])[0] for _ in range(N)); sn = min(run(cn, SUMMARY, [])[0] for _ in range(N))
assert sorted(cb.execute(SUMMARY).fetchall()) == sorted(cn.execute(SUMMARY).fetchall())
print(f"feed summaries: {sb:.1f} -> {sn:.1f} ms")
