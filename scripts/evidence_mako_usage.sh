#!/usr/bin/env bash
# Usage evidence for Mako tools: code search counts, registry lookups (names listed before the lookup).
for q in 'djlint --profile mako' 'djlint mako' 'filename:.pre-commit-config.yaml djlint' 'filename:.pre-commit-config.yaml mako' 'extension:mako tree-sitter' 'mako-lint' 'prettier-plugin-mako'; do
  printf '%s\t' "$(gh api -X GET search/code -f q="$q" -f per_page=1 --jq .total_count 2>/dev/null || echo failed)"
  echo "$q"
  sleep 8
done
for pkg in mako-lint mako-format djlint mako-language-server pymako mako-lsp; do
  printf 'PyPI %s: ' "$pkg"
  curl -s --max-time 30 -o /dev/null -w '%{http_code}\n' "https://pypi.org/pypi/$pkg/json"
done
for pkg in prettier-plugin-mako mako-lint eslint-plugin-mako mako-language-server tree-sitter-mako prettier-plugin-djlint; do
  printf 'npm %s: ' "$pkg"
  curl -s --max-time 30 "https://registry.npmjs.org/$pkg" | python3 -c 'import json,sys
try:
  d=json.load(sys.stdin); print(d.get("name"), d.get("dist-tags",{}).get("latest"), d.get("time",{}).get("modified"))
except Exception as e: print("none")'
done
for q in mako; do
  printf 'open-vsx %s: ' "$q"
  curl -s --max-time 30 "https://open-vsx.org/api/-/search?query=$q&size=10" | python3 -c 'import json,sys
d=json.load(sys.stdin)
for e in d.get("extensions",[]): print(e.get("namespaceName")+"."+e.get("name"), e.get("downloadCount"), e.get("averageRating"))'
done
