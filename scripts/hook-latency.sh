#!/usr/bin/env bash
# Wall time of `sudsy hook post-edit` on one edited file, a cold process each run, as Claude Code
# starts it after every Edit: each FILE is committed in a scratch git repository, one line is
# changed, then the hook runs 11 times (median and min) next to `sudsy --version` (process start)
# and `sudsy lint FILE` (no `--new`). Default: the optimized dev build of the slot.
#   scripts/hook-latency.sh [--sudsy PATH] [FILE...]
set -euo pipefail
cd "$(dirname "$0")/.."
slot=${CARGO_TARGET_DIR:-$(cat .cargo/tmp/SLOT)}
sudsy=$slot/debug/sudsy
if [ "${1:-}" = --sudsy ]; then sudsy=$2; shift 2; fi
files=("$@")
[ ${#files[@]} -gt 0 ] || files=(bench/inputs/py/django-core-paginator.py bench/inputs/js/mui-button.js
  bench/inputs/go/hugo-page.go bench/inputs/md/testing.md)
work=$slot/hook-latency; rm -rf "$work"; mkdir -p "$work"
g() { git -C "$repo" -c user.name=t -c user.email=t@t -c commit.gpgsign=false "$@"; }
ms() { # CMD... -> "median min" in milliseconds over 11 runs
  local runs=() s
  for _ in $(seq 11); do
    s=$(date +%s%N); "$@" >/dev/null 2>&1 || true
    runs+=($((($(date +%s%N) - s) / 1000000)))
  done
  printf '%s\n' "${runs[@]}" | sort -n | awk '{a[NR]=$1} END {print a[6], a[1]}'
}
lint() { (cd "$repo" && "$sudsy" lint "$name"); }
hook() { printf '{"cwd":"%s","hook_event_name":"PostToolUse","tool_name":"Edit","tool_input":{"file_path":"%s"}}' \
  "$repo" "$repo/$name" | "$sudsy" hook post-edit; }
echo "sudsy: $sudsy"
echo "--version (process start): $(ms "$sudsy" --version) ms (median min)"
for f in "${files[@]}"; do
  name=$(basename "$f"); repo=$work/${name%.*}; mkdir -p "$repo"; cp "$f" "$repo/"
  g init -q; g add -A; g commit -qm base
  printf '\n' >>"$repo/$name"
  echo "$f ($(wc -c <"$f") bytes): hook post-edit $(ms hook) ms, lint $(ms lint) ms"
done
