#!/usr/bin/env bash
# Agent host profile setup, after setup.sh (needs its Node): caveman CLI (smaller tool output,
# saved scripts); no telemetry, no Auto routing. Idempotent; runs at `project add` and every boot.
set -euo pipefail
export PATH="$HOME/.local/node/bin:$PATH"
if ! caveman --version 2>/dev/null | grep -q '4\.1\.0'; then
  DO_NOT_TRACK=1 npm install -g --no-fund --no-audit @caveman-ai/cli@4.1.0
fi
DO_NOT_TRACK=1 caveman telemetry off
DO_NOT_TRACK=1 caveman off routing --yes  # without --yes it only prints what it would change
