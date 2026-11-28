#!/usr/bin/env bash
# usage: batch.sh MODEL REPS PAR EXP[/COND]...   e.g. batch.sh haiku 3 4 E1 E4/B
set -uo pipefail
MODEL="${1:?usage: batch.sh MODEL REPS PAR EXP[/COND]...}"; REPS="${2:?}"; PAR="${3:?}"; shift 3
ROOT="$(cd "$(dirname "$0")" && pwd)"
for spec in "$@"; do
  exp="${spec%%/*}"
  if [ "$spec" = "$exp" ]; then conds="$(ls "$ROOT/tools/variants/$exp")"; else conds="${spec#*/}"; fi
  for c in $conds; do for r in $(seq 1 "$REPS"); do echo "$exp $c $r $MODEL"; done; done
done | xargs -P "$PAR" -L1 "$ROOT/run.sh"
