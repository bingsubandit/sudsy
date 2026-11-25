# Design: Dockerfile format evidence

The Dockerfile formatter ([../formatting/dockerfile.md](../formatting/dockerfile.md)) predates [../development/no-reference-tool.md](../development/no-reference-tool.md). This page holds the evidence that page asks for: sources, corpus, decision table, numbers. Reproduce with `python3 -I scripts/evidence.py dockerfile <sudsy> <work-dir> <dockerfmt>` (read-only on the corpus; it prints every table below) and `scripts/parity.sh stability dockerfile`. No formatter change is part of it.

## Sources
Looked up 2026-11-25 (GitHub API, npm, PyPI, Homebrew, VS Code Marketplace, conform.nvim and mega-linter files):

| Tool | Repository | Last push | Stars | Use | Changes | Runs on Linux here |
|---|---|---|---|---|---|---|
| dockerfmt 1.0.0 | reteps/dockerfmt | 2026-11-14 | 629 | npm `@reteps/dockerfmt` 1.55M a month (dependency of prettier-plugin-sh, 1.88M); release assets 899k downloads; Homebrew 163 (30 days); listed by conform.nvim; mentioned in 62 `.pre-commit-config.yaml` and 18 workflow files (hadolint: 4,568 and 12,512) | keyword and `AS` case, stage name case, blanks, JSON arrays, continuation layout (`&&` at line start), comments, blank lines, legacy `ENV` and `MAINTAINER` rewritten, shell of `RUN` formatted with mvdan/sh | yes (Go binary, 0.05 s a file) |
| docker-language-server (formatter) | docker/docker-language-server | 2026-11-19 | 202 | VS Code `docker.docker` 2.9M installs; `ms-azuretools.vscode-containers` 20M | "rudimentary": indentation | not tried (LSP) |
| dockerfile-utils `format` | rcjsuen/dockerfile-utils | 2025-07-21 | 37 | npm 2.4M a month (dependency of the language server of `ms-azuretools.vscode-docker`, 53M installs, last update 2025-05) | indentation of continued lines (tabs by default) | yes (npm 0.16.3) |
| dprint-plugin-dockerfile 0.8.0 | dprint/dprint-plugin-dockerfile | 2026-11-06 | 16 | npm 32k a month; 1 `dprint.json` found | comment blank, legacy `ENV`, JSON arrays, blank lines | yes (dprint wasm) |
| hadolint, BuildKit checks, mega-linter | | | | lint only (mega-linter's Dockerfile linter is hadolint) | no formatter | |

Verdict: dockerfmt is the primary tool of the format leg. It is the only tool that lays out a whole file, the only one named by conform.nvim, and its engine sits under the most used Prettier plugin for it. The editor formatters have more installs but only move indentation; dockerfmt covers that part too. It is not registered as a parity leg.

## Corpus
563 files of 26 repositories (22 projects: the four docker-library images are one project, moby and buildx one), pinned by commit: `tests/conformance/fetch-dockerfile-corpus.sh` (dotnet-docker, docker-library, moby, buildx) and `fetch-dockerfile-evidence-corpus.sh` (19 more repositories, chosen before any run: node, grafana, prometheus, tensorflow, pytorch, airflow, home-assistant, kind, next.js, cockroach, mastodon, traefik, istio, argo-cd, opentelemetry-demo, microservices-demo, nextcloud, beats, vault; envoy was dropped from the fetch: its only `Dockerfile.in` is a template, so no file is left and the fetch refuses an empty repository). Files named `Dockerfile.tmpl`, `Dockerfile.in` and `Dockerfile.dockerignore` are not Dockerfiles and are not fetched. The stability group `dockerfile` reads both folders.

Split: the 22 projects are shuffled with seed 20261125 (`random.Random`); the first 11 decide, the other 11 are held out. Deciding: cockroach, docker-library, dotnet, grafana, istio, kind, microservices-demo, next.js, nodejs, tensorflow, vault. Held out: airflow, argo-cd, beats, home-assistant, mastodon, moby, nextcloud, opentelemetry-demo, prometheus, pytorch, traefik.

## Against dockerfmt
dockerfmt 1.0.0, defaults, run from an empty folder (it reads `.editorconfig`). 552 files are accepted by both tools (Sudsy refuses 9, dockerfmt 2). Files with equal output:

| Files | Held out (213) | All (552) |
|---|---|---|
| equal | 97 (45.5%) | 207 (37.5%) |
| equal when only shell code of `RUN`, `CMD`, `ENTRYPOINT`, `HEALTHCHECK` differs | 178 (83.6%) | 382 (69.2%) |
| and legacy `ENV`, `COPY` JSON form and `MAINTAINER` rewrites left out | 206 (96.7%) | 542 (98.2%) |
| left unchanged by Sudsy / by dockerfmt | 103 (48.4%) / 72 (33.8%) | 176 / 134 |

Causes of difference (occurrences, files; `[shell]` is shell code, which Sudsy keeps as written):

| Cause | Occurrences | Files |
|---|---|---|
| `ENV key value` written as `key=value` | 577 | 125 |
| `[shell]` indent of continued lines | 363 | 183 |
| `[shell]` `&&`, `\|\|` moved from line end to line start | 171 | 88 |
| `[shell]` lines joined or split | 120 | 70 |
| `COPY` JSON array written as plain arguments | 55 | 37 |
| `[shell]` separators, redirect blanks, `case` layout | 44 | 32 |
| comment: blank after `#` | 11 | 3 |
| `COPY` continuation layout | 8 | 3 |
| `[shell]` `HEALTHCHECK` lines | 5 | 5 |
| blank lines between instructions | 3 | 3 |
| `[shell]` `CMD` lines | 2 | 2 |
| `ADD` continuation layout | 1 | 1 |
| `MAINTAINER` written as `LABEL` | 1 | 1 |

(13 causes occur; the 15 largest are all of them.) Shell code alone is the whole difference of 175 of 552 files (31.7%); without the shell the formatter is at 69% and, apart from the three rewrites, at 98%. dockerfmt's shell layout is mvdan/sh plus its own continuation layout, so the difference is mostly layout of line breaks (indent, operator at the line start, joins), not token rewrites (44 occurrences). Sudsy's Shell formatter (shfmt layout, [../formatting/shell.md](../formatting/shell.md)) can read the `RUN` body (the `embed` rows of `grammars/dockerfile.grammar` parse it; their `format` flag is not set), but its rules put an operator behind a continuation at the line end and indent continuation lines one level, so the 171 and 363 occurrences stay different unless the layout is a data option of the shared shell mechanism; that layout is the decision rows below, not shfmt's.

## Decision table
Variants are counted per project on the deciding half (one vote per project; a project votes its most frequent variant). Agreement is the share of held-out occurrences that already have the form Sudsy writes.

| Choice | Votes (deciding) | Rule result | Sudsy writes | Agrees | Held-out occurrences with it |
|---|---|---|---|---|---|
| keyword case | 11 upper | 100%: default | upper | yes | 3699 of 3699 (100%) |
| `AS` case | 9 upper, 1 lower (10 projects) | 90%: default | upper | yes | 273 of 320 (85.3%) |
| blanks between arguments | 11 single | 100%: default | one | yes | 3631 of 3699 (98.2%) |
| JSON array | 10 `["a", "b"]`, 1 `[ "a" ]` | 91%: default | `["a", "b"]` | yes | 146 of 174 (83.9%) |
| blank before the backslash | 11 one | 100%: default | one | yes | 3486 of 3714 (93.9%) |
| continuation indent | 7 four, 2 two, 1 eight, 1 tab | 64%: default and option | four, all continued lines alike | yes | 1979 of 3714 (53.3%) |
| comment in a continuation | 5 indented, 1 column 0 (6 projects) | 83%: default | indented | yes | 62 of 115 (53.9%) |
| blank lines | 11 at most one | 100%: default | at most one | yes | 1678 of 1724 (97.3%) |
| final newline | 11 yes | 100%: default | yes | yes | 213 of 221 (96.4%) |
| heredoc body | 1 project (58 held-out heredocs) | no variant | as written | n/a | 58 of 58 |
| `&&`, `\|\|` at line end or start | 7 start, 4 end | 64%: default and option | as written | not decided | 312 start, 228 end |

`agreement` on the held-out half: 15,225 of 17,438 occurrences (87.3%) pooled over the ten decided rows; the mean of the rows is 86.2%. Two rows are far below 80% on their own: continuation indent (53.3%) and comment in a continuation (53.9%). The first is a defect, not taste: the corpus keeps nested levels in continued lines (27% of the held-out lines are at 8 spaces: package lists under `apt-get install`, `sed -e` arguments, `case` arms), and Sudsy flattens every continued line to one level. The rule gives the default "four spaces for the first level"; the evidence says nothing for flattening, so a nested level is kept (re-based) instead. Comments inside a continuation: only docker-library (951 of 1,095 comment lines) keeps them at column 0, the five other projects indent them; Sudsy indents them like the lines around.

`stable` (`scripts/parity.sh stability dockerfile`, debug build with the safety check): 554 of 563 files (98.4%); every file that parses is stable (second format changes nothing, nothing copied through). The 9 that are not fail to parse: see bugs.md.

Harm: 30 of the 376 changed files, chosen with seed 20261126 and read. No change of meaning. Visible harm: continued lines lose their nested indentation (14 files: `apt-get install` packages, `sed -e` lists, aligned `\` columns); comments at column 0 inside a continuation are indented (3, the docker-library style); the rest is wanted (`as` to `AS`, `[ "x" ]` to `["x"]`, double blanks, trailing blanks).

## Meaning oracle
`dockerfile-ast` (rcjsuen, npm, Node only, no Docker) parses Dockerfiles to instructions and arguments. Compared before and after `sudsy format`: keyword upper-cased, arguments joined and stripped of blanks, `AS` case folded. 563 of 563 files are the same (11 are copies of files Sudsy refused). BuildKit's parser (what dockerfmt and the language server use) is Go and no Go toolchain is installed here; hadolint's parser is a Haskell library with no JSON output of the tree. The script of the run is not part of the repository (a Node dependency for one check); the safety check of the formatter remains the gate.

## Left out
Bugs found, with inputs, are in [../bugs.md](../bugs.md). Choices of taste for the user: `&&` at the line start (64%, as dockerfmt does) with an option; keep legacy `ENV key value`, `MAINTAINER` and `COPY` JSON as written (they are rewrites of syntax, not layout; dockerfmt changes them); format the shell of `RUN` or leave it.
