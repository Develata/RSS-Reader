# Worst case for the list: nothing matches, so the walk along the sort-key index cannot stop at
# the limit. Copies the two 250k fixtures, gives every entry a publication time, then lists and
# counts with a cutoff later than all of them (everything automatically archived).
# Usage (in the directory holding final.py and the fixtures): python3 -I nomatch.py
import shutil, sqlite3, sys, time

for src in ("base250k.db", "w250k.db"):
    shutil.copy(src, "nm_" + src)
    c = sqlite3.connect("nm_" + src)
    c.execute("UPDATE entries SET published_at = created_at WHERE published_at IS NULL")
    c.commit()
    c.execute("pragma wal_checkpoint(truncate)")
    c.close()
sys.argv = ["final.py", "nm_base250k.db", "nm_w250k.db", "1"]
src = open("final.py").read()
src = src[: src.index("report = {}")]
exec(src.replace('CUT = "2026-08-12T00:00:00Z"', 'CUT = "2030-01-01T00:00:00Z"'))
for k in [("all", "all", "excl_auto", "all", False), ("all", "all", "only_auto", "all", False)]:
    for name, build in (("list", list_sql), ("count", count_sql)):  # noqa: F821
        sb, pb = build(k, False)
        sn, pn = build(k, True)
        tb = tn = 1e9
        for _ in range(5):
            t = time.perf_counter(); rb = cb.execute(sb, pb).fetchall(); tb = min(tb, time.perf_counter() - t)  # noqa: F821
            t = time.perf_counter(); rn = cn.execute(sn, pn).fetchall(); tn = min(tn, time.perf_counter() - t)  # noqa: F821
        assert rb == rn
        rows = len(rn) if name == "list" else rn[0][0]
        print(f"{name} {k} rows={rows}: {tb*1000:.1f} -> {tn*1000:.1f} ms")
