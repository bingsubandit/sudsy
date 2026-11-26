#!/usr/bin/env bash
# Batch 2 selection of the Rust evidence (docs/ai/design/rust.md): the first 1000 items (ten pages of 100) of the GitHub code
# search for `extension:rs` (relevance order, fixed before any file read): repo, path and blob sha, tab-separated. Written to
# tests/conformance/rust-search.tsv (committed, pins the corpus). Vendored and generated code is not filtered here: the
# corpus fetch and the evidence module do that.
for page in 1 2 3 4 5 6 7 8 9 10; do
  gh api -X GET search/code -f q='extension:rs' -f per_page=100 -f page=$page --jq '.items[] | [.repository.full_name, .path, .sha] | @tsv'
  sleep 8
done
