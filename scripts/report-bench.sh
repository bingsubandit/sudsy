#!/usr/bin/env bash
# Wall time of printing a large lint report, per output format (best of 5, release CLI).
# Input: FILES YAML files of LINES lines `kNNNN: yes`, one `ambiguous-booleans` finding with an
# unsafe fix per line. `github` prints one line per finding and is the floor (lint plus a minimal
# report); the other formats are compared with it.
# Usage: scripts/report-bench.sh [FILES] [LINES]   (default 20 x 1000 = 20,000 findings)
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
slot=$(cat "$root/.cargo/tmp/SLOT" 2>/dev/null || echo "${CARGO_TARGET_DIR:?set CARGO_TARGET_DIR or .cargo/tmp/SLOT}")
files=${1:-20}
lines=${2:-1000}
CARGO_TARGET_DIR=$slot cargo build --release -q -p cli
bin=$slot/release/sudsy
dir=$slot/report-bench
rm -rf "$dir"
mkdir -p "$dir"
printf 'lint:\n  rules:\n    ambiguous-booleans: warn\n' >"$dir/sudsy.yaml"
python3 - "$dir" "$files" "$lines" <<'EOF'
import sys
d, files, lines = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
body = "".join(f"k{i:05}: yes\n" for i in range(lines))
for f in range(files):
    with open(f"{d}/f{f:03}.yaml", "w", newline="\n") as out:
        out.write(body)
EOF
cd "$dir"
run() {
    local best=
    for _ in 1 2 3 4 5; do
        local s e t
        s=$(date +%s%N)
        "$bin" lint "$@" . >/dev/null 2>&1 || true
        e=$(date +%s%N)
        t=$(((e - s) / 1000000))
        if [ -z "$best" ] || [ "$t" -lt "$best" ]; then best=$t; fi
    done
    echo "$best"
}
echo "findings=$((files * lines))"
for args in "--format github" "--format concise" "--format human" "--format human --rule ambiguous-booleans" "--format json"; do
    # shellcheck disable=SC2086
    printf '%-45s %6s ms  %s bytes\n' "$args" "$(run $args)" "$("$bin" lint $args . 2>/dev/null | wc -c)"
done
