**Repo:** servo/rust-url
**Title:** Parsing is quadratic in the number of `..` segments when the authority is long (`last_slash_can_be_removed` scans the whole serialization)

## Summary

Each `..` path segment calls `last_slash_can_be_removed`. That function runs `rfind('/')` over the entire serialization so far, not just the path. When the path is already at the root, there is no `/` after the authority, so every `..` rescans the whole scheme, userinfo and host. An input with a long userinfo (or host) followed by many `..` segments therefore parses in Θ(len(authority) × number of `..`). That is quadratic in the input length, while the output stays short.

We hit this while hardening an RSS reader that parses untrusted feed links with `url` (directly and through `feed-rs`). Extrapolating the timings below, a single 4 MB feed link of this shape would keep a refresh busy for about a minute. We currently carry the one-function change suggested below as a `[patch.crates-io]` on 2.5.8. It passes the crate's unit tests and the WPT URL test data, and turns the repro below linear (n=65536: 252 ms → 2.4 ms).

## Reproduction

`url = "=2.5.8"`, `cargo run --release`:

```rust
use std::time::Instant;
use url::Url;

fn main() {
    for n in [8_192usize, 16_384, 32_768, 65_536] {
        // A long userinfo, then `..` segments that cannot climb above the root.
        let input = format!("https://{}@example.com/{}", "a".repeat(n), "../".repeat(n));
        let t = Instant::now();
        let url = Url::parse(&input).unwrap();
        println!("n={n:>6}  input={:>7} B  output={:>6} B  {:>10.3?}", input.len(), url.as_str().len(), t.elapsed());
    }
    for n in [8_192usize, 16_384, 32_768, 65_536] {
        // Control: same number of `..` segments, short authority.
        let input = format!("https://example.com/{}", "../".repeat(n));
        let t = Instant::now();
        Url::parse(&input).unwrap();
        println!("control n={n:>6}  input={:>7} B  {:>10.3?}", input.len(), t.elapsed());
    }
}
```

Output (release, x86_64 Windows):

```
n=  8192  input=  32789 B  output=  8213 B     3.459ms
n= 16384  input=  65557 B  output= 16405 B    16.974ms
n= 32768  input= 131093 B  output= 32789 B    72.955ms
n= 65536  input= 262165 B  output= 65557 B   251.908ms
control n=  8192  input=  24596 B   327.600µs
control n= 16384  input=  49172 B   665.600µs
control n= 32768  input=  98324 B     1.275ms
control n= 65536  input= 196628 B     2.612ms
```

Doubling the input multiplies the time by 4 to 5, while the control stays linear. Because TAB/LF/CR are stripped before path parsing and `%2e` counts as a dot, a pre-filter on the raw input has to repeat that normalization to be reliable.

## Where the time goes

In `src/parser.rs` (2.5.8), the `..` branch of path parsing (line 1324) calls `last_slash_can_be_removed(&self.serialization, path_start)`. That function (lines 1384–1394) does `serialization[..serialization.len() - 1].rfind('/')` and only afterwards compares the result with `path_start`. When the path has no earlier `/`, the reverse search runs through the whole authority before it fails.

## Suggestion

The function already receives `path_start`, so the search can be limited to the path: `serialization[path_start..serialization.len() - 1].rfind('/')`, adding `path_start` back to the index. A slash before `path_start` is rejected by the existing `segment_before_start >= path_start` check anyway, so the result is unchanged. Each `..` then costs O(length of the path), and in the root case O(1). The overall parse becomes linear.
