#!/usr/bin/env bash
# Pins the default-branch head of each named Mako project and counts its .mako files (threshold check only).
for r in sqlalchemy/mako sqlalchemy/alembic Pylons/pyramid_mako Pylons/pyramid Pylons/pylons galaxyproject/galaxy reddit/reddit sqlalchemy/sqlalchemy Pylons/pyramid_debugtoolbar kallithea-scm/kallithea; do
  b=$(gh api "repos/$r" --jq .default_branch 2>/dev/null)
  s=$(gh api "repos/$r/commits/$b" --jq .sha 2>/dev/null)
  n=$(gh api "repos/$r/git/trees/$s?recursive=1" --jq '[.tree[] | select(.path|endswith(".mako"))] | length' 2>/dev/null)
  echo "$r $b ${s:-none} mako=${n:-none}"
  sleep 1
done
