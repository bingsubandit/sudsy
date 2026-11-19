#!/usr/bin/env bash
# Differential test against an upstream tool. Usage: scripts/parity.sh [format|coverage] <tool> | --pin <format tool> | --test
# `--pin`: a format run that also prints the pins (hashes of Sudsy's output) of the `tool-wrong` entries, for the lead to paste;
# `--test`: the runner's own tests (scripts/test_parity.py, no tool or build needed).
# Lint mode runs the tool and `sudsy lint` on tests/parity/<tool>/ (or the rule fixtures) and compares
# rule (mapped through docs/ai/parity/<tool>.yaml), line and column; format mode compares `sudsy format`
# with the tool's output; coverage mode reports the share of the tool's default-set findings on its corpus that
# Sudsy also reports (docs/ai/development/parity.md). Prints every mismatch; exit 1 on any. The tools, their versions' catalogs
# and how they are installed are the entries of scripts/parity_tools.json (docs/ai/development/parity.md).
set -euo pipefail
cd "$(dirname "$0")/.."
. scripts/lib/slot.sh
tdir=${CARGO_TARGET_DIR:-$PWD/target}
mode=lint
extra=()
test=
if [ "${1:-}" = --test ]; then test=1; shift; fi
if [ "${1:-}" = --pin ]; then mode=format; extra=(--pin); shift; fi
if [ "${1:-}" = format ]; then mode=format; shift; fi
if [ "${1:-}" = coverage ]; then extra=(--coverage); shift; fi
if [ "${1:-}" = --proof ]; then extra+=(--proof); shift; fi  # coverage: the formatter findings that remain after `sudsy format`
tool=${1:-}
py=
for candidate in python3 python; do
  if "$candidate" -c 'import sys' >/dev/null 2>&1; then py=$candidate; break; fi
done
[ -n "$py" ] || { echo "parity.sh: python 3 is required" >&2; exit 2; }
if [ -z "$test" ]; then
  "$py" -c 'import json,sys; sys.exit(sys.argv[2] not in json.load(open("scripts/parity_tools.json"))[sys.argv[1]])' "$mode" "$tool" ||
    { echo "usage: scripts/parity.sh [format] <$("$py" -c 'import json,sys; print("|".join(json.load(open("scripts/parity_tools.json"))[sys.argv[1]]))' "$mode")>" >&2; exit 2; }
  cargo build --quiet -p cli
  sudsy=$tdir/debug/sudsy; [ -x "$sudsy" ] || sudsy=$sudsy.exe
fi
# The comparison needs PyYAML (the lint catalogs, the differences.yaml of both modes).
venv="$tdir/parity/venv"
if [ ! -d "$venv" ]; then
  "$py" -m venv "$venv"
  vpy="$venv/bin/python"; [ -x "$vpy" ] || vpy="$venv/Scripts/python.exe"
  "$vpy" -m pip install --quiet pyyaml
fi
py="$venv/bin/python"; [ -x "$py" ] || py="$venv/Scripts/python.exe"
[ -z "$test" ] || exec "$py" scripts/test_parity.py
exec "$py" "scripts/parity_$mode.py" "$tool" "$sudsy" "$tdir" "${extra[@]}"
