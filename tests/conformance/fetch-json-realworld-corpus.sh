#!/usr/bin/env bash
# Fetches the pinned real-world JSON corpus into tests/conformance/corpus/json-real/<repo> (gitignored):
# SchemaStore's JSON schemas and their test inputs, and the package.json, tsconfig.json and settings files
# (JSON with comments) of Microsoft's VS Code extension samples.
source "$(dirname "$0")/tarball-corpus.sh"
fetch_tarballs json-real <<'LIST'
SchemaStore/schemastore 526b7d5314d1aa566262277accf6681b4eb3bf9a */src/schemas/json/*.json */src/test/*.json
microsoft/vscode-extension-samples b284b915f1afbc8835bccda2ce4e198c23b719f0 *.json
LIST
