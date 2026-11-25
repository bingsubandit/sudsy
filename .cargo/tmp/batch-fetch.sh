#!/bin/bash
# Fetch the .bat/.cmd files of the pinned repositories (sparse, shallow) into CORPUS (default slot dir).
# Usage: batch-fetch.sh PINS-FILE   (lines: dir url sha; sha HEAD resolves and prints the commit)
set -u
CORPUS=${CORPUS:-/scratch/slots/sudsy/8/corpus-batch}
mkdir -p "$CORPUS"
while read -r dir url sha; do
  [ -z "$dir" ] && continue
  case "$dir" in \#*) continue;; esac
  d="$CORPUS/$dir"
  if [ ! -d "$d/.git" ]; then
    git init -q "$d"
    git -C "$d" remote add origin "$url"
    git -C "$d" config core.sparseCheckout true
    printf '*.bat\n*.cmd\n*.BAT\n*.CMD\n*.Bat\n*.Cmd\nLICENSE*\nLICENCE*\nCOPYING*\n' > "$d/.git/info/sparse-checkout"
    git -C "$d" config extensions.partialClone origin
    git -C "$d" config remote.origin.promisor true
    git -C "$d" config remote.origin.partialclonefilter blob:none
  fi
  ref=$sha
  [ "$sha" = HEAD ] && ref=HEAD
  if git -C "$d" fetch -q --depth 1 --filter=blob:none origin "$ref" 2>/dev/null; then
    git -C "$d" checkout -q -f FETCH_HEAD 2>/dev/null
    echo "$dir $url $(git -C "$d" rev-parse HEAD) $(find "$d" -path "$d/.git" -prune -o \( -iname '*.bat' -o -iname '*.cmd' \) -type f -print | wc -l)"
  else
    echo "$dir $url FAILED 0"
  fi
done < "$1"
