#!/usr/bin/env bash
# Usage evidence of the Helm legs (docs/ai/design/helm.md): code search totals of the tools chart repositories run (helm lint,
# kubeconform, chart-testing, helm-docs, prettier, yamllint), and registry lookups of Helm template formatters and linters
# (names listed before each lookup). Needs `gh` logged in; code search counts are totals of the API, not a sample.
# Usage: bash scripts/evidence_helm_usage.sh
set -uo pipefail
count() {
  gh api -X GET search/code -f q="$1" -f per_page=1 --jq .total_count 2>/dev/null || echo "failed"
  sleep 8
}
echo "--- code search (total)"
for q in \
  'path:.github/workflows "helm lint"' \
  'path:.github/workflows "helm template"' \
  'path:.github/workflows kubeconform' \
  'path:.github/workflows "chart-testing"' \
  'path:.github/workflows "ct lint"' \
  'path:.github/workflows prettier' \
  'path:.github/workflows yamllint' \
  'filename:.pre-commit-config.yaml "helm-lint"' \
  'filename:.pre-commit-config.yaml "helm-docs"' \
  'filename:.pre-commit-config.yaml prettier' \
  'filename:.pre-commit-config.yaml yamllint' \
  'filename:.helmignore' \
  'filename:Chart.yaml "apiVersion: v2"' \
  ; do
  printf '%s\t%s\n' "$(count "$q")" "$q"
done
echo "--- registry lookups (PyPI and npm): template formatters and linters for Helm templates"
for pkg in helm-lint helm-format helmfmt chart-testing; do
  printf 'PyPI %s: ' "$pkg"
  curl -s --max-time 30 -o /dev/null -w '%{http_code}\n' "https://pypi.org/pypi/$pkg/json"
done
for pkg in prettier-plugin-helm prettier-plugin-go-template helm-language-server helm-lint eslint-plugin-helm tree-sitter-go-template; do
  printf 'npm %s: ' "$pkg"
  curl -s --max-time 30 "https://registry.npmjs.org/$pkg" | python3 -c 'import json,sys
try:
  d=json.load(sys.stdin); print(d.get("name"), d.get("dist-tags",{}).get("latest"), d.get("time",{}).get("modified"))
except Exception as e: print("none")'
done
