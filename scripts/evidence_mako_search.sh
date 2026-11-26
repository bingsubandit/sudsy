#!/usr/bin/env bash
# Batch 2 selection of the Mako evidence (docs/ai/design/mako.md): the first 1000 items (ten pages of 100) of the GitHub code
# search for `extension:mako` (relevance order, fixed before any file read): repo, path and blob sha, tab-separated. Written to
# tests/conformance/mako-search.tsv (committed, pins the corpus). `.mak` is not searched: 276,992 hits, mostly Makefile fragments.
for page in 1 2 3 4 5 6 7 8 9 10; do
  gh api -X GET search/code -f q='extension:mako' -f per_page=100 -f page=$page --jq '.items[] | [.repository.full_name, .path, .sha] | @tsv'
  sleep 8
done
