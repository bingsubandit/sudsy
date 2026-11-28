#!/usr/bin/env bash
# usage: setup.sh EXP COND REP  -> creates runs/EXP-COND-REP and prints its path
set -euo pipefail
EXP="${1:?usage: setup.sh EXP COND REP}"; COND="${2:?}"; REP="${3:?}"
ROOT="$(cd "$(dirname "$0")" && pwd)"
RUN="$ROOT/runs/$EXP-$COND-$REP"
VAR="$ROOT/tools/variants/$EXP/$COND"
[ -d "$VAR" ] || { echo "no variant $EXP/$COND" >&2; exit 1; }
rm -rf "$RUN"; mkdir -p "$ROOT/runs"
cp -a "$ROOT/base" "$RUN"
find "$RUN" -name __pycache__ -type d -prune -exec rm -rf {} +
[ -d "$ROOT/extras/$EXP" ] && cp -a "$ROOT/extras/$EXP/." "$RUN/"
for f in "$VAR"/* "$VAR"/.[!.]*; do
  [ -e "$f" ] || continue
  case "$(basename "$f")" in TASK_TOOLS.md|NEEDS|SETUP|TASK_NOTE.md) continue;; esac
  cp -a "$f" "$RUN/"
done
if [ -f "$VAR/NEEDS" ]; then
  mkdir -p "$RUN/.mock"
  while read -r n; do [ -n "$n" ] && cp -a "$ROOT/tools/common/$n" "$RUN/.mock/$n"; done < "$VAR/NEEDS"
fi
[ -f "$RUN/.claude-settings.json" ] && sed -i "s#__RUN__#$RUN#g" "$RUN/.claude-settings.json"
[ -f "$VAR/SETUP" ] && (cd "$RUN" && bash "$VAR/SETUP")
{
  cat "$ROOT/tasks/$EXP/TASK.md"
  [ -f "$VAR/TASK_NOTE.md" ] && { echo; cat "$VAR/TASK_NOTE.md"; }
  echo; echo "## Tools"; cat "$VAR/TASK_TOOLS.md"
  echo; echo "## Running tests"
  echo "\`python3 -m tests\` runs all tests (also \`python3 tests/run.py\`); \`python3 -m tests test_NAME\` runs one test module."
  echo "Work only inside this directory and do not commit. Keep your change focused: touch only what the task needs."
  echo ""
  echo "Shell access is limited: run one simple command per Bash call (no pipes, ;, &&, heredocs, cd or python3 -c). Allowed: the ./ scripts listed above, python3 -m tests, python3 tests/run.py, git diff, git status, ls, grep, cat, head, tail, wc, sed, mkdir. black and ruff are not available directly. Use the Edit and Write tools to change files."
} > "$RUN/TASK.md"
: > "$RUN/.calls.log"
cd "$RUN"
git init -q -b main
git -c user.name=exp -c user.email=exp@example.com add -A
git -c user.name=exp -c user.email=exp@example.com commit -q -m "base $EXP/$COND"
git tag exp-base
git config user.name exp; git config user.email exp@example.com
echo "$RUN"
