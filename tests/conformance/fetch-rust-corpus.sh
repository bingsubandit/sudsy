#!/usr/bin/env bash
# Fetches the pinned files of the Rust evidence (docs/ai/design/rust.md) into tests/conformance/corpus/rust/<owner>__<repo>
# (gitignored). Scope: `*.rs`, and the files that set the format and the lints of their project (`Cargo.toml`, `rustfmt.toml`,
# `.rustfmt.toml`, `clippy.toml`, `.clippy.toml`), which the evidence needs for the edition and the configuration.
# Batch 1: 12 public projects of different authors and sizes (command line tools, async runtime, serialization, errors,
# CLI parsing, HTTP, terminal, documentation tool, the analyzer), pinned by commit (default-branch heads of 2026-11-26), listed
# before any result. Batch 2: one blob per line of tests/conformance/rust-search.tsv (GitHub code search `extension:rs`, first
# 1000 items, pinned by blob sha) in a repository that batch 1 does not hold; a single file has no Cargo.toml, so its edition
# is not known (rustfmt runs as edition 2021, see scripts/evidence_rust_tools.py). The cap of `scripts/evidence_rust.py`
# (30 files per project, the first by path) is the only limit.
export TARBALL_LENIENT=1 TARBALL_OWNER=1
source "$(dirname "$0")/tarball-corpus.sh"
P='*.rs */Cargo.toml */rustfmt.toml */.rustfmt.toml */clippy.toml */.clippy.toml !*/target/* !*/node_modules/* !*/vendor/* !*/.git/*'
for line in \
  'BurntSushi/ripgrep 3e85c6c148fdce38a99ceeb9d2780dc7b03b4a4f $P' \
  'sharkdp/bat c3fe143626d09b6b276093dc34fc44c51b17a9a9 $P' \
  'sharkdp/fd 2e8648f10fe55af6258ab90ab889eb8b96033379 $P' \
  'tokio-rs/tokio 854013a8cb7d1397e9d037315592dc1d2d55187d $P' \
  'serde-rs/serde 5be4690236e16d8a16d80f76703e59091ac3be2e $P' \
  'dtolnay/anyhow c89c9b82b155f701a20eb4a9d3609ad09df5a62f $P' \
  'clap-rs/clap 4201754a400f676c9744da66383c89eb9f41af88 $P' \
  'hyperium/hyper 55247767429fb29753f724b467869aa6203db532 $P' \
  'alacritty/alacritty 791efff6d74c66bee390c259e322626ca93aadb6 $P' \
  'starship/starship 9110d4b7bed9e2ab37e045b2340a1927ec86b6a7 $P' \
  'rust-lang/mdBook bd27e1236eafc6a779ea7e936e4c2fd7615806a3 $P' \
  'rust-lang/rust-analyzer 9c74e78e01d7cd31663f4ecf71e65b1cc4b9ccb8 $P' \
  ; do
  fetch_tarballs rust <<<"${line//\$P/$P}" || echo "no Rust file: ${line%% *}"
done

# Batch 2: a blob per search line, unless the repository is one of batch 1.
DEST="$(cd "$(dirname "$0")" && pwd)/corpus/rust"
NAMED=' BurntSushi/ripgrep sharkdp/bat sharkdp/fd tokio-rs/tokio serde-rs/serde dtolnay/anyhow clap-rs/clap hyperium/hyper alacritty/alacritty starship/starship rust-lang/mdBook rust-lang/rust-analyzer '
while IFS=$'\t' read -r repo path sha; do
  case "$NAMED" in *" $repo "*) continue ;; esac
  case "$path" in *.rs) ;; *) continue ;; esac
  out="$DEST/${repo%%/*}__${repo#*/}/$path"
  [ -f "$out" ] && continue
  mkdir -p "$(dirname "$out")"
  gh api "repos/$repo/git/blobs/$sha" --jq .content | base64 -d >"$out" || { rm -f "$out"; echo "no blob: $repo $path"; }
done <"$(dirname "$0")/rust-search.tsv"
echo "batch 2 ready: $DEST"
