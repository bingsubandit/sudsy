#!/usr/bin/env bash
# Pins the default-branch head of each named Helm chart repository and counts its chart templates (threshold check only): the
# `templates/` files of a pinned tree (`.yaml`, `.yml`, `.tpl`). A project joins the corpus when it holds at least 3.
# Usage: bash scripts/evidence_helm_pins.sh
for r in prometheus-community/helm-charts grafana/helm-charts kubernetes/ingress-nginx argoproj/argo-helm jetstack/cert-manager \
  traefik/traefik-helm-chart elastic/helm-charts open-telemetry/opentelemetry-helm-charts kyverno/kyverno \
  external-secrets/external-secrets hashicorp/vault-helm DataDog/helm-charts nginxinc/kubernetes-ingress rook/rook \
  redpanda-data/helm-charts apache/airflow goharbor/harbor-helm kubernetes/dashboard temporalio/helm-charts \
  zalando/postgres-operator cilium/cilium; do
  b=$(gh api "repos/$r" --jq .default_branch 2>/dev/null)
  s=$(gh api "repos/$r/commits/$b" --jq .sha 2>/dev/null)
  n=$(gh api "repos/$r/git/trees/$s?recursive=1" --jq '[.tree[] | select(.path|test("(^|/)templates/[^/]*\\.(ya?ml|tpl)$"))] | length' 2>/dev/null)
  echo "$r $b ${s:-none} templates=${n:-none}"
  sleep 1
done
