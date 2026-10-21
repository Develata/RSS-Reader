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

The result is unchanged: any slash found before `path_start` already made the function return `false`, and the
Windows drive-letter check still sees the same index.

## Verification

- The crate's own test suite, including the WPT URL test data in `tests/`, was run against the patched copy.
- `rssr-domain` has a canary test that parses a long-authority, many-`..` URL. Without this patch it would take
  minutes.

## Removal

Drop the `[patch.crates-io]` entry, the `exclude`, and this directory once an upstream `url` release contains an
equivalent fix. Issue draft: `docs/upstream-drafts/rust-url-dot-segments.md`.
