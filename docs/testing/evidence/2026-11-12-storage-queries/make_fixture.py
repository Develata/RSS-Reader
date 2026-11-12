# Builds a library fixture from the real migrations: 200 feeds (5 soft-deleted), N entries with
# realistic row widths (title, url, author, ~400-char summary), 10% unread, 2% starred,
# 3% manually archived, 5% NULL published_at, dates spread over ~6 years.
import sqlite3, random, os, sys, datetime
MIG = os.path.join(os.path.dirname(os.path.abspath(__file__)), "../../../../crates/rssr-infra/src/store/migrations/")
n = int(sys.argv[1]); path = sys.argv[2]
for suffix in ("", "-wal", "-shm"):
    if os.path.exists(path + suffix): os.remove(path + suffix)
c = sqlite3.connect(path)
c.execute("pragma journal_mode=wal")
c.create_function("fold_for_search", 1, lambda s: None if s is None else s.lower(), deterministic=True)
for f in ("0001_baseline.sql", "0002_manual_archive.sql"):
    c.executescript(open(MIG + f).read())
random.seed(7)
c.execute("begin")
for i in range(1, 201):
    c.execute("insert into feeds(url,title,is_deleted,created_at,updated_at) values(?,?,?,?,?)",
              (f"https://feed{i}.example/rss", f"Feed {i}", 1 if i % 40 == 0 else 0,
               "2020-01-01T00:00:00Z", "2020-01-01T00:00:00Z"))
base = datetime.datetime(2020, 11, 12)
words = "lorem ipsum dolor sit amet consectetur adipiscing elit sed do eiusmod tempor".split()
rows = []
for i in range(n):
    d = base + datetime.timedelta(minutes=random.randint(0, 6 * 365 * 24 * 60))
    s = d.strftime("%Y-%m-%dT%H:%M:%SZ")
    t = f"Article {i} about topic {random.randint(0, 500)} and more words"
    summary = " ".join(random.choice(words) for _ in range(70))
    rows.append((random.randint(1, 200), f"ext-{i}", f"dedup-{i}", f"https://site.example/post/{i}",
                 t, t.lower(), "Some Author", summary, None if random.random() < 0.05 else s, s,
                 1 if random.random() < 0.9 else 0, 1 if random.random() < 0.02 else 0,
                 1 if random.random() < 0.03 else 0, s, s))
c.executemany("""insert into entries(feed_id,external_id,dedup_key,url,title,title_search,author,summary,
    published_at,first_seen_at,has_content,is_read,is_starred,is_archived,created_at,updated_at)
    values(?,?,?,?,?,?,?,?,?,?,1,?,?,?,?,?)""", rows)
c.execute("commit")
c.execute("pragma wal_checkpoint(truncate)")
print(path, n, os.path.getsize(path) // 1_000_000, "MB")
