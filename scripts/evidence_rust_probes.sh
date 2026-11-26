#!/usr/bin/env bash
# rustfmt's verdict on each edition probe (scripts/evidence_rust_parsers/probes, the edition is in the file name): ok when
# rustfmt parses it in that edition, error otherwise. The oracle of the probes column of docs/ai/design/rust.md.
# Usage: bash scripts/evidence_rust_probes.sh   (the repository toolchain, rust-toolchain.toml: 1.99.0)
set -uo pipefail
DIR="$(cd "$(dirname "$0")" && pwd)/evidence_rust_parsers/probes"
for f in "$DIR"/*.rs; do
  b="$(basename "$f" .rs)"
  ed="${b##*.}"
  if RUSTUP_TOOLCHAIN=1.99.0 rustfmt --edition "$ed" --emit stdout <"$f" >/dev/null 2>&1; then r=ok; else r=error; fi
  printf '%s %s\n' "$b" "$r"
done
