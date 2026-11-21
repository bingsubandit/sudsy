#!/usr/bin/env bash
# Fetches the pinned real-world YAML corpus into tests/conformance/corpus/yaml-real/<repo> (gitignored): the
# Kubernetes manifests of kubernetes/website (docs examples) and the Compose files of docker/awesome-compose.
source "$(dirname "$0")/tarball-corpus.sh"
fetch_tarballs yaml-real <<'LIST'
kubernetes/website 542bad2ab92dd6ee7916b2b916d84cda64c77f6c */content/en/examples/*.yaml
docker/awesome-compose 503c0dcef31b07fb77b2d4c5556bfafdf86f4f1a *.yaml *.yml
LIST
