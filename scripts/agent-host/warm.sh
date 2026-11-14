#!/usr/bin/env bash
# Agent host warm-up after boot (low priority): builds the test binaries in slot 1, so the first
# agent's gate starts warm. /scratch is wiped at every boot, so this runs every boot.
set -euo pipefail
cd "$(dirname "$0")/../.."
export CARGO_TARGET_DIR=/scratch/slots/sudsy/1
cargo test --workspace --no-run --quiet
