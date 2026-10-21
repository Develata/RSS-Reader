# Vendored `url` 2.5.8 with one local patch

This directory is the crates.io release `url` 2.5.8, copied unchanged except for one function. It is wired in through
`[patch.crates-io]` in the workspace `Cargo.toml` and excluded from the workspace, so it is not linted or tested as
part of RSS-Reader.

## The patch

`src/parser.rs`, `Parser::last_slash_can_be_removed`: search for the slash only in `serialization[path_start..]`
instead of the whole serialization.

Upstream runs `rfind('/')` over the whole URL and then rejects any result before `path_start`. When the path is at
the root, that search walks through the entire scheme, userinfo and host before failing. It runs once for every `..`
segment, so an input with a long authority and many `..` segments parses in Θ(authority length × number of `..`).
That is quadratic in the input. Feed links, `xml:base` values, subscription URLs and redirect targets all reach this
code with remote data, both inside feed-rs/reqwest and in our own code.

The result is unchanged for every call the parser can make: any slash found before `path_start` already made the
function return `false`, and the Windows drive-letter check still sees the same index. `path_start` always comes from
the length of the serialization built so far, so it is a char boundary. Independent review (Codex, 2026-10-21) agreed,
and noted that the dot-segment work is now linear by amortization.

## Verification

- The crate's own test suite, including the WPT URL test data in `tests/`, was run against the patched copy.
- `rssr-domain` has a canary test (`url_guard::tests::dot_segments_parse_in_linear_time`) that parses a 4 MB
  long-authority, many-`..` URL on a separate thread with a 60 s limit. It passes in about 0.5 s with the patch. With the
  unpatched file restored, it fails at 60 s with a message pointing here (checked by mutation on 2026-10-21).
- The vendored crate is excluded from the workspace, so its own tests are not part of `cargo test --workspace`. When
  syncing this copy with a new upstream release, run them from a copy outside the repository (cargo refuses to run
  them in place under the workspace root).

## Removal

Drop the `[patch.crates-io]` entry, the `exclude`, and this directory once an upstream `url` release contains an
equivalent fix. Issue draft: `docs/upstream-drafts/rust-url-dot-segments.md`.
