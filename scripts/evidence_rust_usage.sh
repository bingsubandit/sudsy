#!/usr/bin/env bash
# Usage evidence of the Rust legs (docs/ai/design/rust.md): GitHub code search hit counts for rustfmt configuration keys and
# clippy lint levels, workflow and pre-commit use of the two tools, and the maintenance facts of the parser candidates
# (crates.io, GitHub). Needs `gh` logged in; code search counts are totals of the API, not a sample.
# Usage: bash scripts/evidence_rust_usage.sh
set -uo pipefail
count() {
  gh api -X GET search/code -f q="$1" -f per_page=1 --jq .total_count 2>/dev/null || echo "failed"
  sleep 8
}
echo "--- rustfmt configuration files and keys (code search total)"
for q in \
  'filename:rustfmt.toml' \
  'filename:.rustfmt.toml' \
  'filename:rustfmt.toml max_width' \
  'filename:rustfmt.toml use_small_heuristics' \
  'filename:rustfmt.toml style_edition' \
  'filename:rustfmt.toml imports_granularity' \
  'filename:rustfmt.toml group_imports' \
  'filename:rustfmt.toml newline_style' \
  ; do
  printf '%s\t%s\n' "$(count "$q")" "$q"
done
echo "--- clippy levels and tools in workflows and pre-commit (code search total)"
for q in \
  'filename:Cargo.toml "[lints.clippy]"' \
  'filename:Cargo.toml "[workspace.lints.clippy]"' \
  'filename:clippy.toml' \
  'clippy::pedantic' \
  'clippy::nursery' \
  'clippy::unwrap_used' \
  'path:.github/workflows cargo clippy' \
  'path:.github/workflows cargo fmt' \
  'filename:.pre-commit-config.yaml rustfmt' \
  'filename:.pre-commit-config.yaml clippy' \
  ; do
  printf '%s\t%s\n' "$(count "$q")" "$q"
done
echo "--- parser candidates: crates.io (max stable version, last update, downloads, recent downloads, repository)"
for c in tree-sitter tree-sitter-rust syn ra_ap_syntax; do
  printf '%s: ' "$c"
  curl -s --max-time 30 -H 'User-Agent: sudsy-evidence' "https://crates.io/api/v1/crates/$c" | python3 -c 'import json,sys
c=json.load(sys.stdin)["crate"]
print(c.get("max_stable_version") or c.get("max_version"), c["updated_at"], c["downloads"], c["recent_downloads"], c.get("repository"))'
  sleep 1
done
echo "--- parser candidates: GitHub (stars, pushed, archived)"
for r in tree-sitter/tree-sitter-rust dtolnay/syn rust-lang/rust-analyzer tree-sitter/tree-sitter; do
  printf '%s: ' "$r"
  gh api "repos/$r" --jq '[.stargazers_count, .pushed_at, .archived] | @tsv'
  sleep 1
done
