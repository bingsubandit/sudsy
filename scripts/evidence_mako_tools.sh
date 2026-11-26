#!/usr/bin/env bash
# Tool evidence of the Mako legs (docs/ai/design/mako.md): djLint with its `mako` profile on a copy of the corpus, format
# (`--check`: the files it would change) and lint (`--lint`: the rule codes it reports). Needs djlint (pip install djlint==1.54.0)
# on PATH or in $DJLINT. Usage: bash scripts/evidence_mako_tools.sh <work-dir>   (the work dir is emptied and filled)
set -uo pipefail
W="${1:?usage: evidence_mako_tools.sh <work-dir>}"
DJ="${DJLINT:-djlint}"
CORPUS="$(cd "$(dirname "$0")/.." && pwd)/tests/conformance/corpus/mako"
rm -rf "$W/djlint"; mkdir -p "$W/djlint"
cp -r "$CORPUS/." "$W/djlint/"
# djLint stops at the first file that is not UTF-8 (Mako's own cp1251 test template): such files are left out and counted.
while IFS= read -r -d '' f; do
  iconv -f utf-8 -t utf-8 "$f" >/dev/null 2>&1 || { echo "not UTF-8, left out: $f"; rm -f -- "$f"; }
done < <(find "$W/djlint" -type f -name '*.mako' -print0)
echo "files: $(find "$W/djlint" -type f -name '*.mako' | wc -l)"
echo "--- format (djlint --check --profile mako)"
"$DJ" --check --profile mako --extension mako "$W/djlint" >"$W/format.txt" 2>&1
echo "exit $?"; tail -2 "$W/format.txt"
echo "--- lint (djlint --lint --profile mako): rule codes"
"$DJ" --lint --profile mako --extension mako "$W/djlint" >"$W/lint.txt" 2>&1
echo "exit $?"; tail -2 "$W/lint.txt"
grep -o ' H[0-9]\{3\} ' "$W/lint.txt" | sort | uniq -c | sort -rn | head -40
