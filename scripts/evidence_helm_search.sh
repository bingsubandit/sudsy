#!/usr/bin/env bash
# Batch 2 selection of the Helm evidence (docs/ai/design/helm.md): the first 1000 items (ten pages of 100) of the GitHub code
# search `"{{ .Values" path:templates` (relevance order, fixed before any file read): repo, path and blob sha, tab-separated.
# Written to tests/conformance/helm-search.tsv (committed, pins the corpus). The phrase keeps Go templates of charts and
# leaves out other `{{ }}` templates (cookiecutter, Ansible); `path:templates` is the folder of a chart's templates.
# Usage: bash scripts/evidence_helm_search.sh > tests/conformance/helm-search.tsv
for page in 1 2 3 4 5 6 7 8 9 10; do
  gh api -X GET search/code -f q='"{{ .Values" path:templates' -f per_page=100 -f page=$page --jq '.items[] | [.repository.full_name, .path, .sha] | @tsv'
  sleep 8
done
