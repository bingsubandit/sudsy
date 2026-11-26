#!/usr/bin/env bash
# Fetches the pinned files of the Procfile evidence (docs/ai/design/procfile.md) into tests/conformance/corpus/procfile/<owner>__<repo>
# (gitignored). Process files are `Procfile`, `Procfile.dev` and `Procfile.local` (Heroku, foreman, honcho, overmind, hivemind,
# dokku, herokuish); other names (`procfile.rst`, `Procfile.md`) are documentation and are left out.
# Batch 1: 45 public projects pinned by commit, listed before any result (Heroku sample apps and buildpacks, whose test fixtures
# hold many Procfiles; the process runners; well known Rails, Django and Node applications). Three had no default-branch head and
# were dropped before any fetch. Batch 2: the first 300 items of the GitHub code search for `filename:Procfile`, pinned in
# tests/conformance/procfile-search.tsv (repository, path, blob sha; written by the search, not by looking at file content).
# A Procfile is one short file, so most repositories of batch 2 hold one file; the corpus keeps every file, the cap of
# `scripts/evidence_procfile.py` (60 per project, the first by path) is the only limit.
export TARBALL_LENIENT=1 TARBALL_OWNER=1
source "$(dirname "$0")/tarball-corpus.sh"
P='*/Procfile */Procfile.dev */Procfile.local !*/node_modules/* !*/vendor/* !*/.git/*'
for line in \
  'heroku/python-getting-started 561bc55e4aaf22f98c040611fd613994547b31bb $P' \
  'heroku/node-js-getting-started d33649d5edae61fc766fe6d47e530c475c05a7a0 $P' \
  'heroku/ruby-getting-started ca4d54f4071967352690d0c5673a2060ca1f570a $P' \
  'heroku/java-getting-started 03e69b85ee88f39e01ccb44be71f088486912a33 $P' \
  'heroku/go-getting-started ac7df2489d66d682932300505875841d71f2c50e $P' \
  'heroku/scala-getting-started 0f849476e36336bc4d7c46b5a69bd010567f9689 $P' \
  'heroku/php-getting-started 3aeae3fecf761822217682c1455034a24458b30d $P' \
  'heroku/heroku-buildpack-ruby 1057d6620f694858ae1efad3795c2151621126fd $P' \
  'heroku/heroku-buildpack-nodejs cbecbf47e4fd3af20716f69d6c35d0eac23e193c $P' \
  'heroku/heroku-buildpack-python 069e9184147e6d95190f851dbdcf06337ec244ee $P' \
  'heroku/heroku-buildpack-java 900a31d669f5605ab22c7c4446a5b6a594deea9b $P' \
  'heroku/heroku-buildpack-go a0774d53b6953dea6a369fede30df8b6262c4198 $P' \
  'heroku/heroku-buildpack-php c65bc07a641fad6233ffbbc05f1a95ecc69e80b7 $P' \
  'heroku/heroku-buildpack-scala 02e0cdbea7015bba24cbfc12d60fb4abbc9d861c $P' \
  'heroku/heroku-buildpack-clojure 8dbea0db13fc3a48305bfbe01b4ba50560b4a030 $P' \
  'heroku/heroku-buildpack-nginx ac8f89bebade00f746ecc4cfa991f3fdd35b230e $P' \
  'heroku/heroku-buildpack-multi 308249e81d61d0549504b3ce6e27ec6f168a7393 $P' \
  'heroku/heroku-buildpack-static d9044589483fbf3b5378362925c3fb124c4c6851 $P' \
  'heroku/heroku-buildpack-elixir 866920027d662e3394e07afa1ec4876b2d877c19 $P' \
  'ddollar/foreman f65ddba83932bd4670e014389d6e27ea1e20b469 $P' \
  'nickstenning/honcho 2f47b723587c65ecbadc99e674b515b20a749179 $P' \
  'dokku/dokku 96320ff377b4242e037878bcc13af66f3df8f144 $P' \
  'gliderlabs/herokuish faade6ff2eee432196b4e951e8e739f6d349b52b $P' \
  'DarthSim/overmind f57a6d00fc8b2eafd1d12014283eea25c7132d6e $P' \
  'DarthSim/hivemind d8b34a08299a644f98afe332521271990acc60f7 $P' \
  'diaspora/diaspora d382e334db882976a42e39a90e00cfe2619c3398 $P' \
  'forem/forem 3dcba730246909089f090400b855268f50d4eed5 $P' \
  'mastodon/mastodon 6c29cb4b9c1f8b3f9a1be23f0efb9c5ac64155a3 $P' \
  'discourse/discourse 6d1101515e4dfa0bea4e058456554128bef2c696 $P' \
  'rubygems/rubygems.org 231011b48fe108686bc63c6d49d3f1121aec8263 $P' \
  'publiclab/plots2 5e1d8c2bac1c5d2ad7faf532162a72c6f65c9898 $P' \
  'codetriage/codetriage 3292712244fd39916986b204b61adc2066b78602 $P' \
  'sharetribe/sharetribe 7d6f37630a28a532e1966f7708281b6803d1c841 $P' \
  'pydanny/cookiecutter-django 45baa3dc901c81fc9981e1759be74c4df6f65345 $P' \
  'solidusio/solidus 600c7f73f66da3c46a52b028d19b1b7a229eb644 $P' \
  'spree/spree 7c38466c396e29b964547a5f8414d1bc8fab3f01 $P' \
  'getsentry/sentry 0d74e59b9391c1b34ca7c72a4c4778aea6814275 $P' \
  'socketio/chat-example 07a183a618aea3e1c7f63c8d9bb1fcb78a1e99e8 $P' \
  'codeforamerica/howto 476f18e2ea3362bdddfe3ddca34acd7008d9bf5d $P' \
  'thoughtbot/suspenders b282f49dabcebba7e595d9956e651ce3a73f48ad $P' \
  'kaminari/kaminari e3d7834dbafef40ad48935e0623aec2ca93eb7d8 $P' \
  'trailblazer/trailblazer 33f180432ed63c998820b550d2b632d3d694676e $P' \
  'rails/rails-observers 389b577322d8b17336730b2dc4e179060a23c8e7 $P' \
  'elastic/elasticsearch-rails 41f4291407f4cd1b9f2c2b574e05853920086955 $P' \
  'Shopify/liquid 15a2e32d186f713643fe86202da8da44a7bb058b $P' \
  ; do
  fetch_tarballs procfile <<<"${line//\$P/$P}" || echo "no Procfile: ${line%% *}"
done

# Batch 2: one blob per line of procfile-search.tsv whose file name is a process file (the search also returns documentation).
DEST="$(cd "$(dirname "$0")" && pwd)/corpus/procfile"
while IFS=$'\t' read -r repo path sha; do
  case "$(basename "$path")" in Procfile|Procfile.dev|Procfile.local) ;; *) continue ;; esac
  out="$DEST/${repo%%/*}__${repo#*/}/$path"
  [ -f "$out" ] && continue
  mkdir -p "$(dirname "$out")"
  gh api "repos/$repo/git/blobs/$sha" --jq .content | base64 -d >"$out" || { rm -f "$out"; echo "no blob: $repo $path"; }
done <"$(dirname "$0")/procfile-search.tsv"
echo "batch 2 ready: $DEST"
