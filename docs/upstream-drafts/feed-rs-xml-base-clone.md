**Filed:** feed-rs/feed-rs#301 (2026-10-21)
**Repo:** feed-rs/feed-rs
**Title:** A long `xml:base` is cloned into every element, so parse time grows with base length × element count

## Summary

`ElementSource::xml_base_fetch` returns `state.base_uris.last().map(|(_, uri)| uri.clone())`, and every yielded `Element` gets its own clone of the current base `Url`. A feed with a long `xml:base` on the root element and many child elements therefore copies the base once per element. A 1 MiB `xml:base` with 2,000 entries takes about 9 s to parse, against 13 ms with a short base.

We parse untrusted feeds in an RSS reader, where this shows up as a refresh that can be stalled by a single feed. We now pre-scan the raw XML before calling feed-rs and reject feeds whose total base copying (in-scope `xml:base` bytes summed over elements) exceeds a constant multiple of the input size. That pre-scan has to mirror feed-rs's scoping rules, which is fragile; a fix in the library would be better for everyone.

## Reproduction

`feed-rs = "=2.4.0"`, `cargo run --release`:

```rust
use std::time::Instant;

fn atom(base_len: usize, entries: usize) -> String {
    let body: String = (0..entries)
        .map(|i| format!(r#"<entry><id>{i}</id><title>x</title><link href="p{i}"/></entry>"#))
        .collect();
    format!(
        r#"<feed xmlns="http://www.w3.org/2005/Atom" xml:base="https://example.com/{}/"><id>f</id><title>t</title>{body}</feed>"#,
        "a".repeat(base_len)
    )
}

fn main() {
    for entries in [250usize, 500, 1_000, 2_000] {
        let xml = atom(1 << 20, entries);
        let t = Instant::now();
        let feed = feed_rs::parser::parse(xml.as_bytes()).unwrap();
        println!("xml:base=1 MiB entries={entries:>5}  bytes={:>8}  entries_parsed={}  {:>10.3?}", xml.len(), feed.entries.len(), t.elapsed());
    }
    for entries in [250usize, 500, 1_000, 2_000] {
        let xml = atom(16, entries);
        let t = Instant::now();
        feed_rs::parser::parse(xml.as_bytes()).unwrap();
        println!("control xml:base=16 B entries={entries:>5}  bytes={:>8}  {:>10.3?}", xml.len(), t.elapsed());
    }
}
```

Output (release, x86_64 Windows):

```
xml:base=1 MiB entries=  250  bytes= 1063964  entries_parsed=250   867.184ms
xml:base=1 MiB entries=  500  bytes= 1079464  entries_parsed=500      2.191s
xml:base=1 MiB entries= 1000  bytes= 1110464  entries_parsed=1000      5.112s
xml:base=1 MiB entries= 2000  bytes= 1174464  entries_parsed=2000      8.882s
control xml:base=16 B entries=  250  bytes=   15404     1.326ms
control xml:base=16 B entries=  500  bytes=   30904     3.315ms
control xml:base=16 B entries= 1000  bytes=   61904     4.971ms
control xml:base=16 B entries= 2000  bytes=  125904    13.454ms
```

The input grows by only about 10% across the four rows, while parse time grows ten-fold, proportional to the number of elements.

## Suggestion

Share the base instead of cloning it per element, for example by keeping `Rc<Url>` (or `Arc<Url>`) on the `base_uris` stack and in `Element::xml_base`. That makes each element O(1) regardless of base length. Separately, every link is resolved with `Url::join` against that base, which copies the base once per link. Sharing the base doesn't remove that cost, so it may be worth documenting or capping the base length.

Related: link and URL attributes are passed to `url::Url::parse`/`join`, which is quadratic in the number of `..` segments for inputs with a long authority (reported upstream as servo/rust-url#1163). That one is best fixed in `url` itself; we carry a local patch for it.
