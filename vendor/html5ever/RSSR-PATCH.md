# RSS-Reader local html5ever patch

Base: crates.io `html5ever 0.39.0`, upstream commit
`ce64836c685025a5fef0860fa2e9c80b2683e8d0`, directory `html5ever/`.
Upstream MIT and Apache-2.0 licenses are retained. The normalized registry manifest
and upstream sources/tests are retained; this is not a new parser implementation.

Changes are restricted to two source files:

- `src/tokenizer/mod.rs`: after 16 distinct attributes, use a per-tag `HashSet`
  for duplicate detection. Preserve attribute order, first-value-wins, duplicate
  errors, and the duplicate flag. Release the index on emission and discarded tags.
  Ordinary small tags allocate no index. See
  [upstream issue 796](https://github.com/servo/html5ever/issues/796).
  The SIMD feature helper is compiled only on the same architectures as its
  caller, avoiding the upstream unused-function warning on wasm.
- `src/tree_builder/mod.rs`: expose `active_formatting_len()` in constant time,
  including markers. RSS-Reader's existing metering preparse uses this observation;
  tree construction behavior is unchanged. See
  [upstream issue 801](https://github.com/servo/html5ever/issues/801).

As of 2026-10-28, both hotspots remain in the released `html5ever-v0.40.1` sources.
Remove the tokenizer patch when an upstream release fixes attribute deduplication;
remove the observer when upstream provides equivalent accounting/cancellation.
Do not upgrade this copy without checking ammonia's resolved parser version and
rerunning reader stress, corpus, tokenizer chunking, native and wasm checks. The
other transitive html5ever version in the workspace is not patched by this entry.

Regression coverage lives in RSS-Reader's `test_reader_html_stress` and
`test_reader_tokenizer` targets and `html::parse_budget` unit tests. Performance is
measured by the `reader_html_probe` example; it is not a wall-clock CI assertion.
