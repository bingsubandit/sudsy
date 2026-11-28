#!/usr/bin/env bash
# usage: run.sh EXP COND REP MODEL   (real headless subject run; scored; appended to results.jsonl)
set -uo pipefail
EXP="${1:?usage: run.sh EXP COND REP MODEL}"; COND="${2:?}"; REP="${3:?}"; MODEL="${4:?}"
ROOT="$(cd "$(dirname "$0")" && pwd)"
RUN="$("$ROOT/setup.sh" "$EXP" "$COND" "$REP")" || exit 1
[ -n "${SMOKE_TASK:-}" ] && printf "%s\n" "$SMOKE_TASK" > "$RUN/TASK.md"
OUT="$ROOT/results/$EXP-$COND-$REP"; mkdir -p "$OUT"
ALLOW="Read,Edit,Write,Glob,Grep,Bash(./*),Bash(python3 -m tests*),Bash(python3 tests/run.py*),Bash(git diff*),Bash(git status*),Bash(ls*),Bash(grep*),Bash(cat*),Bash(head *),Bash(wc *),Bash(sed *),Bash(mkdir *),Bash(tail *)"
cd "$RUN"
ARGS=(-p "$(cat TASK.md)" --model "$MODEL" --output-format json --allowedTools "$ALLOW")
[ -f .claude-settings.json ] && ARGS+=(--settings "$RUN/.claude-settings.json")
timeout 900 claude "${ARGS[@]}" > "$OUT/result.json" 2> "$OUT/stderr.txt" < /dev/null
echo "exit=$?" >> "$OUT/stderr.txt"
cd "$ROOT"
python3 -I _score/merge.py "$RUN" "$OUT/result.json" >> "$ROOT/results.jsonl"
tail -n 1 "$ROOT/results.jsonl"
