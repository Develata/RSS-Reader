# Rechecks every shape that was more than 1 ms slower in a first pass at 250k: compares the two
# plans and times both versions again (7 interleaved runs, minimum). Produced
# recheck-slower-250k.txt. Usage (beside final.py and the fixtures): python3 -I recheck.py
import sys, time

sys.argv = ["final.py", "base250k.db", "w250k.db", "1"]
src = open("final.py").read()
exec(src[: src.index("report = {}")])


def plan(c, s, p):
    return " | ".join(r[3] for r in c.execute("EXPLAIN QUERY PLAN " + s, p))


def best(c, s, p, n):
    b = 1e9
    for _ in range(n):
        t = time.perf_counter(); c.execute(s, p).fetchall(); b = min(b, time.perf_counter() - t)
    return b * 1000


for name, build in (("list", list_sql), ("count", count_sql)):  # noqa: F821
    slower = []
    for k in combos:  # noqa: F821
        sb, pb = build(k, False)
        sn, pn = build(k, True)
        same = plan(cb, sb, pb) == plan(cn, sn, pn)  # noqa: F821
        tb = tn = 1e9
        for _ in range(5):
            tb = min(tb, best(cb, sb, pb, 1)); tn = min(tn, best(cn, sn, pn, 1))  # noqa: F821
        if tn - tb > 1:
            tb2 = tn2 = 1e9
            for _ in range(7):
                tb2 = min(tb2, best(cb, sb, pb, 1)); tn2 = min(tn2, best(cn, sn, pn, 1))  # noqa: F821
            slower.append((k, same, tb, tn, tb2, tn2))
    print(f"{name}: {len(slower)} slower >1ms in pass 1")
    for k, same, tb, tn, tb2, tn2 in slower:
        print(f"  {k} plan {'same' if same else 'CHANGED'}: pass1 {tb:.1f}->{tn:.1f}, recheck {tb2:.1f}->{tn2:.1f}")
