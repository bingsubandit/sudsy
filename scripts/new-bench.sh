#!/usr/bin/env bash
# Wall time of `sudsy lint --new` against `sudsy lint` (best of 5, release CLI) on a git repository
# of FILES YAML files of LINES lines (`kNNNNN: yes`, one `ambiguous-booleans` finding per line),
# with 0, CHANGED and all files edited since HEAD (a line inserted at the top and one new finding).
# `--new` reads the changed files from git and lints their old text once more.
# Usage: scripts/new-bench.sh [FILES] [LINES] [CHANGED]   (default 2000 x 100, 20 changed)
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
slot=$(cat "$root/.cargo/tmp/SLOT" 2>/dev/null || echo "${CARGO_TARGET_DIR:?set CARGO_TARGET_DIR or .cargo/tmp/SLOT}")
files=${1:-2000}
lines=${2:-100}
changed=${3:-20}
CARGO_TARGET_DIR=$slot cargo build --release -q -p cli
bin=$slot/release/sudsy
dir=$slot/new-bench
rm -rf "$dir"
mkdir -p "$dir"
printf 'lint:\n  rules:\n    ambiguous-booleans: warn\n' >"$dir/sudsy.yaml"
write() {
    python3 - "$dir" "$files" "$lines" "$1" <<'EOF'
import sys
d, files, lines, edited = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), int(sys.argv[4])
body = "".join(f"k{i:05}: yes\n" for i in range(lines))
for f in range(files):
    with open(f"{d}/f{f:04}.yaml", "w", newline="\n") as out:
        out.write(("a: 1\nz: yes\n" if f < edited else "") + body)
EOF
}
write 0
cd "$dir"
git init -q
git add -A
git -c user.name=bench -c user.email=bench@localhost -c commit.gpgsign=false commit -qm base
run() {
    local best=
    for _ in 1 2 3 4 5; do
        local s e t
        s=$(date +%s%N)
        "$bin" lint --format concise "$@" . >/dev/null 2>&1 || true
        e=$(date +%s%N)
        t=$(((e - s) / 1000000))
        if [ -z "$best" ] || [ "$t" -lt "$best" ]; then best=$t; fi
    done
    echo "$best"
}
echo "files=$files lines=$lines findings=$((files * lines))"
for edited in 0 "$changed" "$files"; do
    write "$edited"
    printf 'changed %5s: lint %6s ms, lint --new %6s ms (%s new)\n' "$edited" "$(run)" "$(run --new)" \
        "$("$bin" lint --format concise --new . 2>/dev/null | wc -l)"
done
