#!/usr/bin/env bash
# Fetches the pinned files of the Helm evidence (docs/ai/design/helm.md) into tests/conformance/corpus/helm/<owner>__<repo>
# (gitignored). Scope: the templates of charts (`templates/*`, any depth: `.yaml`, `.yml` and `.tpl` are read by the evidence),
# and `Chart.yaml` and `values.yaml` of the same trees, which `helm lint` and `helm template` need (scripts/evidence_helm_tools.sh).
# Batch 1: 19 public chart repositories of different authors (monitoring, ingress, certificates, GitOps, databases, operators,
# service meshes, networking), pinned by commit (default-branch heads of 2026-11-26, scripts/evidence_helm_pins.sh) and listed
# before any result. Repositories with fewer than 3 templates (goharbor/harbor-helm, kubernetes/dashboard) are dropped.
# Batch 2: one blob per line of tests/conformance/helm-search.tsv (GitHub code search `"{{ .Values" path:templates`, first 1000
# items, pinned by blob sha) in a repository that batch 1 does not hold. A single blob has no Chart.yaml: it is not a lint input.
# The cap of `scripts/evidence_helm.py` (40 files per project, the first by path) is the only limit on the evidence.
export TARBALL_LENIENT=1 TARBALL_OWNER=1
source "$(dirname "$0")/tarball-corpus.sh"
P='*/templates/* */Chart.yaml */values.yaml !*/node_modules/* !*/vendor/* !*/.git/*'
for line in \
  'prometheus-community/helm-charts 7384a6a2b524862b3729ca310e9cbc0443695bc4 $P' \
  'grafana/helm-charts ad2fee9aacc7dff1933959939c1d548c736ec88f $P' \
  'kubernetes/ingress-nginx dbb11b92ddb77bee9c35e462479129354c84f939 $P' \
  'argoproj/argo-helm 098e5594aa82f1acb036fbbe5434766fced2eb4f $P' \
  'jetstack/cert-manager dae17e52558f6670471043051bf21a2f547d10e6 $P' \
  'traefik/traefik-helm-chart 603785a431d3ea893258e9e00a21ba0309dc1bc1 $P' \
  'elastic/helm-charts 2fd64d0af65f14df7aa01da591919460dabac4b3 $P' \
  'open-telemetry/opentelemetry-helm-charts fd9787614e146fb262d373173d4ade025614ed6b $P' \
  'kyverno/kyverno 84f8c5df17089a267eca78a36146af39e4d0ea0a $P' \
  'external-secrets/external-secrets 32bfb42b6409be8b61f932fe220f13462037b76e $P' \
  'hashicorp/vault-helm 3910503d4baa4dbe18445b45c071881f20fc2435 $P' \
  'DataDog/helm-charts d31df835597672cc8e65515e3c3491017c62bf10 $P' \
  'nginxinc/kubernetes-ingress 2f46a26c8b85b90054610f9eb6ab00e3c8125676 $P' \
  'rook/rook 394e6dba67087967788c4f2fa9bf719f13427fe4 $P' \
  'redpanda-data/helm-charts 80f0e301ff6395a181266420db2158f19ca50405 $P' \
  'apache/airflow ff2575a056ed084a1aa74207f51f10bbdbf61095 $P' \
  'temporalio/helm-charts d214e1f3d555eab46752f640b4eda3359463c166 $P' \
  'zalando/postgres-operator 469ee913e5e0af91d68ca4bc4ba592c789f25931 $P' \
  'cilium/cilium 1cb4c852277a17a5c4861e9bec1b09b46886f3a7 $P' \
  ; do
  fetch_tarballs helm <<<"${line//\$P/$P}" || echo "no Helm template: ${line%% *}"
done

# Batch 2: a blob per search line, unless the repository is one of batch 1.
DEST="$(cd "$(dirname "$0")" && pwd)/corpus/helm"
NAMED=' prometheus-community/helm-charts grafana/helm-charts kubernetes/ingress-nginx argoproj/argo-helm jetstack/cert-manager traefik/traefik-helm-chart elastic/helm-charts open-telemetry/opentelemetry-helm-charts kyverno/kyverno external-secrets/external-secrets hashicorp/vault-helm DataDog/helm-charts nginxinc/kubernetes-ingress rook/rook redpanda-data/helm-charts apache/airflow temporalio/helm-charts zalando/postgres-operator cilium/cilium '
while IFS=$'\t' read -r repo path sha; do
  case "$NAMED" in *" $repo "*) continue ;; esac
  out="$DEST/${repo%%/*}__${repo#*/}/$path"
  [ -f "$out" ] && continue
  mkdir -p "$(dirname "$out")"
  gh api "repos/$repo/git/blobs/$sha" --jq .content | base64 -d >"$out" || { rm -f "$out"; echo "no blob: $repo $path"; }
done <"$(dirname "$0")/helm-search.tsv"
echo "batch 2 ready: $DEST"
