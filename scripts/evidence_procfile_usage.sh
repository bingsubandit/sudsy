#!/usr/bin/env bash
# Usage evidence of the Procfile legs (docs/ai/design/procfile.md): GitHub code search hit counts for the process-file tools
# found by repository search, and the package registries' download counts. Needs `gh` logged in.
# Usage: bash scripts/evidence_procfile_usage.sh
set -uo pipefail
count() {
  gh api -X GET search/code -f q="$1" -f per_page=1 --jq .total_count 2>/dev/null || echo "failed"
  sleep 7
}
for q in \
  'procfile-util' \
  'procfile-lint' \
  'procfile linter' \
  'filename:Procfile procfile-util' \
  'path:.github/workflows Procfile' \
  'filename:.pre-commit-config.yaml procfile' \
  'filename:Procfile.dev overmind' \
  'filename:Procfile jzeuzs' \
  'filename:Procfile "paketo"' \
  ; do
  printf '%s\t%s\n' "$(count "$q")" "$q"
done
for pkg in procfile-util procfile-lint procfile-parser procfile; do
  printf 'npm last month %s: ' "$pkg"
  curl -sSf --max-time 30 "https://api.npmjs.org/downloads/point/last-month/$pkg" 2>/dev/null | sed -n 's/.*"downloads":\([0-9]*\).*/\1/p' || true
  echo
done
for pkg in procfile procfile-util procfile-parser foreman honcho; do
  printf 'PyPI %s: ' "$pkg"
  curl -sSf --max-time 30 "https://pypi.org/pypi/$pkg/json" 2>/dev/null | sed -n 's/.*"name": *"\([^"]*\)".*/\1/p' | head -1
  echo
done
