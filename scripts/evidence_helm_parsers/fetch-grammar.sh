#!/usr/bin/env bash
# Fetches the generated parser of the go-template grammar (ngalaiko/tree-sitter-go-template at commit 06b2fd2, the root grammar
# `gotmpl`, LANGUAGE_VERSION 15) into scripts/evidence_helm_parsers/gotmpl/src (gitignored): parser.c and its tree_sitter
# headers. build.rs of the harness compiles them. The grammar is not on crates.io, and its crate binds a symbol the parser does
# not export (docs/ai/design/helm.md), so the harness builds the parser itself.
set -euo pipefail
D="$(cd "$(dirname "$0")" && pwd)/gotmpl"
rm -rf "$D"; mkdir -p "$D"
curl -sSfL https://codeload.github.com/ngalaiko/tree-sitter-go-template/tar.gz/06b2fd2ac32a7be71cdcf925eeeb868a68e2acde \
  | tar -xz -C "$D" --strip-components=1 --wildcards '*/src/parser.c' '*/src/tree_sitter/*'
echo "grammar ready: $D/src"
