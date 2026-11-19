#!/usr/bin/env bash
# Speed of Sudsy against the tools it replaces (docs/ai/design/speed-comparison.md). Usage: scripts/speed.sh [--out FILE] [<tool>...]
# Every tool with a `speed` key in scripts/parity_tools.json is measured when none is named: `sudsy format --check` and
# `sudsy lint` (the preset that mirrors the tool) against the tool's own check commands on a corpus folder, alternating,
# best of 5 wall seconds and the peak memory of that run. Writes bench/speed.json (a partial run keeps the other rows);
# `cargo xtask codegen` turns it into the README table. Installs the tools into <target>/parity/ and fetches the corpora.
set -euo pipefail
cd "$(dirname "$0")/.."
. scripts/lib/slot.sh
tdir=${CARGO_TARGET_DIR:-$PWD/target}
py=
for candidate in python3 python; do
  if "$candidate" -c 'import sys' >/dev/null 2>&1; then py=$candidate; break; fi
done
[ -n "$py" ] || { echo "speed.sh: python 3 is required" >&2; exit 2; }
cargo build --release --quiet -p cli
# The runner imports parity_format, which needs PyYAML (the venv of parity.sh).
venv="$tdir/parity/venv"
if [ ! -d "$venv" ]; then
  "$py" -m venv "$venv"
  "$venv/bin/python" -m pip install --quiet pyyaml
fi
exec "$venv/bin/python" scripts/speed.py "$tdir/release/sudsy" "$tdir" "$@"
