#!/usr/bin/env bash
# Fetches 19 more pinned projects for the Dockerfile format evidence (docs/ai/design/dockerfile.md) into
# tests/conformance/corpus/dockerfile-evidence/<repo> (gitignored); the projects were chosen before any result was
# seen; templates and ignore lists named Dockerfile.* are not Dockerfiles. Together with fetch-dockerfile-corpus.sh they make the 26 repositories (22 projects) of the evidence; envoy is not in the list: its only file, Dockerfile.in, is a template, so nothing would be left to fetch.
export TARBALL_LENIENT=1
source "$(dirname "$0")/tarball-corpus.sh"
P='*/Dockerfile */Dockerfile.* */*.Dockerfile */*.dockerfile !*/vendor/* !*/node_modules/* !*/testdata/* !*/third_party/* !*/Dockerfile.tmpl !*/Dockerfile.in !*/Dockerfile.dockerignore'
fetch_tarballs dockerfile-evidence <<EOF
nodejs/docker-node 92bbe5ab9dc9b7944c8a57ad7f01388f7fc04902 $P
grafana/grafana ac191e64fdc47345200faaf93673389bbea2928c $P
prometheus/prometheus e56aabdcbec6d8d6621244530654502244d2f5de $P
tensorflow/tensorflow d2f72911d3c24d296ea201492ac565bf78a60f37 $P
pytorch/pytorch a54457b459a5ed6df5ce409f572aeef314fb4145 $P
apache/airflow ec2bd5eafe17052d752ca72c0cfdc4ed534f8ac5 $P
home-assistant/core e2a4d8d9022f09fbcf8d3c549f11c71d73f0f541 $P
kubernetes-sigs/kind b89bd211632684f3dc60187e01ca80c8e3bd2fc3 $P
vercel/next.js ea5a73353a7b8509555d885823971a21b6f4a489 $P
cockroachdb/cockroach d30c905fff79ef825adc96bcc647f1872a90f2ff $P
mastodon/mastodon 6c29cb4b9c1f8b3f9a1be23f0efb9c5ac64155a3 $P
traefik/traefik 8fcaa7cda47c139b76e02ab3d115c168d093ac04 $P
istio/istio e9d2ff691a1d7b84ee490edbd0ea723c03cbf05c $P
argoproj/argo-cd 82f50bbc15a7faf0b3f6d06ff07b3fd582cdb2d1 $P
open-telemetry/opentelemetry-demo a49137190e4c8b01f61b31007bbbd373aa43d606 $P
GoogleCloudPlatform/microservices-demo c7cb6d5bfd6ea3aed768634ebbcb98cb7a523712 $P
nextcloud/docker 18bc14cc9d4d8516fd8cd19034f9493ace636f13 $P
elastic/beats 2c5f84d9345be63537f614259774074c541874bf $P
hashicorp/vault bee4674df62cfb66c8a68345ea19fd8e06a8bd6d $P
EOF
