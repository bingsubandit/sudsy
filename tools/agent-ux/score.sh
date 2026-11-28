#!/usr/bin/env bash
# usage: score.sh RUNDIR -> one JSON line
exec python3 -I "$(cd "$(dirname "$0")" && pwd)/_score/score.py" "$@"
