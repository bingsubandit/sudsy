#!/usr/bin/env bash
# Fetches the pinned files of the Mako evidence (docs/ai/design/mako.md) into tests/conformance/corpus/mako/<owner>__<repo>
# (gitignored). Scope: `*.mako` (Mako's usual extension; `.mak` is left out, see scripts/evidence_mako_search.sh).
# Batch 1: named projects that use Mako (Alembic and SQLAlchemy, Pyramid, Galaxy, Pyramid Debugtoolbar), pinned by commit
# (default-branch heads of 2026-11-26). The list and the threshold were fixed before any corpus file was read: a project joins
# when its pinned tree holds at least 3 `.mako` files. Dropped by the threshold or absent: Pylons/pyramid_mako (2 files),
# Pylons/pylons (0), reddit/reddit (0), sqlalchemy/sqlalchemy (0 files; its docs use Sphinx), kallithea-scm/kallithea (not found).
# Batch 2: one blob per line of tests/conformance/mako-search.tsv (GitHub code search `extension:mako`, first 1000 items,
# pinned by blob sha) in a repository that batch 1 does not hold. A Mako template is one short file, so most repositories of
# batch 2 hold one or two files; the cap of `scripts/evidence_mako.py` (60 per project, the first by path) is the only limit.
export TARBALL_LENIENT=1 TARBALL_OWNER=1
source "$(dirname "$0")/tarball-corpus.sh"
P='*.mako !*/node_modules/* !*/vendor/* !*/.git/*'
for line in \
  'sqlalchemy/mako 14889bc0feacd12102ee61f0769677be2fddbc1b $P' \
  'sqlalchemy/alembic a10b6da895bcde34afbe4c31a14c39b8daac01ca $P' \
  'Pylons/pyramid e9fd014fef30dc6c0059c8fbb0bd214d622e836c $P' \
  'Pylons/pyramid_debugtoolbar 7211efebd6b0ffe163840a142259f2d3e275aed8 $P' \
  'galaxyproject/galaxy ce63095e7a8a9b948594ff7ea69ce9ecf2e4240d $P' \
  ; do
  fetch_tarballs mako <<<"${line//\$P/$P}" || echo "no Mako file: ${line%% *}"
done

# Batch 2: a blob per search line, unless the repository is one of batch 1.
DEST="$(cd "$(dirname "$0")" && pwd)/corpus/mako"
NAMED=' sqlalchemy/mako sqlalchemy/alembic Pylons/pyramid Pylons/pyramid_debugtoolbar galaxyproject/galaxy '
while IFS=$'\t' read -r repo path sha; do
  case "$NAMED" in *" $repo "*) continue ;; esac
  out="$DEST/${repo%%/*}__${repo#*/}/$path"
  [ -f "$out" ] && continue
  mkdir -p "$(dirname "$out")"
  gh api "repos/$repo/git/blobs/$sha" --jq .content | base64 -d >"$out" || { rm -f "$out"; echo "no blob: $repo $path"; }
done <"$(dirname "$0")/mako-search.tsv"
echo "batch 2 ready: $DEST"
