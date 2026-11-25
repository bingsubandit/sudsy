#!/bin/bash
cd /persist/projects/sudsy/worktrees/bridge-cse_01FAPLGUGLGkTv8KC3UkmGGn
git fetch -q origin +refs/heads/main:refs/remotes/origin/main
git log --oneline -5 origin/main
for a in a2710b3e29867bd5e a5d955fcf8f682744 a55d0fda3bb7bde5a ad3dcb2c2f02c6cbf; do
  d=/persist/projects/sudsy/worktrees/agent-$a
  echo "== $a"
  git -C "$d" log --oneline -3
  git -C "$d" status --short | grep -v tests/conformance
done
