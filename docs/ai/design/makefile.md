# Design: Makefile format evidence

The Makefile formatter ([../formatting/makefile.md](../formatting/makefile.md)) predates [../development/no-reference-tool.md](../development/no-reference-tool.md). This page holds the evidence that page asks for: sources, corpus, comparison with the one partial tool, decision table, agreement, stable. Reproduce with `python3 -I scripts/evidence.py makefile <sudsy> <work-dir> [<mbake>]` (the shared harness; the facts of the language are `scripts/evidence_makefile.py`; it is read-only on the corpus and prints every table below) and `scripts/parity.sh stability makefile`. No formatter change is part of it.

## Sources
Looked up 2026-11-25 (GitHub API, PyPI, npm, crates.io, Homebrew, VS Code Marketplace, conform.nvim, nvim-lint and mega-linter files):

| Tool | Repository | Last push | Stars | Use | Changes | Runs on Linux here |
|---|---|---|---|---|---|---|
| mbake 1.5.0 | EbodShojaei/bake | 2026-11-09 | 767 | PyPI 73k downloads a month; listed by conform.nvim and awesome-formatters; a pre-commit hook; VS Code extension (9k installs) | blanks around assignment operators and after the colon, recipe tabs, blanks before backslashes, blank lines, `.PHONY` grouping; no conditional or continuation indent; no parse (a regex tool) | yes (Python) |
| makefmt 0.0.6 | NazmusSayad/makefmt | 2026-10-13 | 1 | npm 86 downloads a month; created 2026-10-12 | aligns assignments, indents conditionals, formats recipes as shell | not tried: no usage |
| makefmt 0.0.3 | donaldgifford/makefmt | 2026-09-17 | 0 | 768 release downloads | not read | not tried: no usage |
| checkmake 0.3.2 | checkmake/checkmake | 2026-11-23 | 1,220 | 888k release downloads; nvim-lint | lint only | yes (the lint leg) |
| unmake 0.0.27 | mcandre/unmake | 2026-11-23 | 120 | crates.io 33k downloads; nvim-lint | lint only | not needed |
| autotools-language-server | Freed-Wu | 2026-07 | | PyPI 1.7k a month | language server for `Makefile.am`, `configure.ac`; no formatter | not tried |
| mega-linter, pre-commit hooks | | | | the Makefile linter of mega-linter is checkmake | no formatter | |

Unreachable: GitHub code search (needs a login), so the count of `.pre-commit-config.yaml` files that name mbake is not known; the VS Code Marketplace gives installs only for the extension of mbake.

Verdict: mbake is the only maintained, used formatter (two orders of magnitude over the others) and is a partial tool: it normalises blanks and blank lines and leaves indentation of conditionals and continued lines as written. It is the comparison tool of the format leg, not a reference that decides layout, and it is not registered as a parity leg. Where it and the corpus disagree with Sudsy, the corpus decides.

## Corpus
1,021 files of 26 projects, pinned by commit: `tests/conformance/fetch-makefile-corpus.sh` (redis, git, lua, prometheus, kubernetes, docker/cli: the 59 files of the conformance corpus) and `fetch-makefile-evidence-corpus.sh` (20 more repositories, chosen before any run: FFmpeg, vim, micropython, postgres, openwrt, ocaml, node, neovim, cockroach, etcd, helm, containerd, terraform, grafana, golangci-lint, numpy, sphinx, u-boot, buildroot, cpython). Every `Makefile`, `GNUmakefile`, `makefile`, `*.mk` and `*.mak`, except vendored and test inputs; the kernel-style trees (u-boot, buildroot, openwrt) are cut to their build system. Downloaded files are only read: the harness and the formatter never run `make`.

Split: the 26 projects are shuffled with seed 20261127 (`random.Random`); the first 13 decide, the other 13 are held out. Deciding: FFmpeg, cockroach, containerd, etcd, helm, kubernetes, micropython, node, numpy, postgres, prometheus, redis, u-boot. Held out: buildroot, cpython, docker-cli, git, golangci-lint, grafana, lua, neovim, ocaml, openwrt, sphinx, terraform, vim.

## Against mbake
mbake 1.5.0, no config, run on stdin (`scripts/parity.sh format mbake`: the `makefile/mbake` preset is the Sudsy side). Of the 1,021 files Sudsy reads 1,008 (the 12 NMake files of vim and `Make_ami.mak`, which is not make syntax, stay out) and mbake 1,019; 1,007 are in both. Files with equal output: 888 (88.2%); `second-format` 0, refused 0. The 119 differing files by cause (each file under its first cause of this order; names in `tests/parity/mbake/differences.yaml`):

| Cause | Files |
|---|---|
| `shell-indent`: mbake re-indents the lines of a shell block in a recipe (two spaces to a tab, nesting by depth) | 59 |
| `blank-lines`: a leading blank line of a file, a run of blank lines after the empty last line of a continuation, an empty `define` body line | 19 |
| `inner-blanks`: blanks inside a line that mbake keeps or adds in ways of its own | 18 |
| `conditional-indent`: mbake moves an indented conditional keyword to column 1 | 10 |
| `subst-ref-colon`: mbake writes a blank after the colon of `$(F:$(SRC)/%=%)` | 8 |
| `continued-indent`: continued lines that mbake leaves where Sudsy sets them by the second line | 5 |

`subst-ref-colon` is a candidate for `tool-wrong`, with an executed proof: for `SRC_PATH = /a` and `INSTALL_FILES = @$(foreach F,$(2),printf '%s' "$(F:$(SRC_PATH)/%=%)" "$(3)"; )`, `make` prints `xy` for `$(call INSTALL_FILES,x,/a/x,y)` before mbake and `/a/xy` after it, which writes `$(F: $(SRC_PATH)/%=%)`. mbake leaves a recipe line that starts inside a quote as written, so a change of meaning inside a quoted string has no proof. The lead decides the `intentional` entry.

## Decision table
Variants are counted per project on the deciding half (one vote per project; a project votes its most frequent variant). Agreement is the share of held-out occurrences that already have the form Sudsy writes. "Rule" is the result of section 3 of the no-reference-tool page: default at 80% of the projects, default and option at 60 to 80%, as written below 60%.

| Choice | Votes (deciding) | Rule result | Sudsy writes | Agrees | Held-out occurrences with it |
|---|---|---|---|---|---|
| blanks around an assignment operator | 10 one each side, 2 several blanks, 1 none | 77%: default and option | one each side | yes | 4,395 of 6,663 (66.0%) |
| alignment of assignments | 11 not aligned, 2 aligned | 85%: default | not aligned | yes | 1,156 of 1,181 (97.9%) |
| blank before the colon of a rule | 13 none | 100%: default | none | yes | 3,548 of 3,655 (97.1%) |
| blank after the colon of a rule | 13 one | 100%: default | one | yes | 2,726 of 2,820 (96.7%) |
| blanks between the words of a rule line | 13 one | 100%: default | one | yes | 2,865 of 2,868 (99.9%) |
| recipe line indent | 13 one tab | 100%: default | one tab | yes | 4,180 of 4,190 (99.8%) |
| conditional body indent (outside a recipe) | 8 none, 3 tab per level (11 projects) | 73%: default none and option | as written; `conditional_body_indent: indent` indents a level | yes | 3,044 of 3,594 (84.7%) as written |
| conditional in a recipe | 12 column 0 | 100%: default | column 0 | yes | 515 of 547 (94.1%) |
| rule or directive continuation: blank before the backslash | 7 one | 100%: default | one | yes | 1,133 of 1,142 (99.2%) |
| rule or directive continuation: indent | 3 tab, 1 four, 1 two, 1 eight | below 60%: as written | as written; `continued_line_indent: indent` writes a tab | yes | as written |
| assignment continuation: blank before the backslash | 7 one, 1 several | 88%: default | as written | not decided | 4,404 of 4,474 are one blank |
| assignment continuation: indent | 3 tab, 2 four, 2 tabs and spaces, 1 eighteen | below 60%: as written | as written | not decided | |
| comment: blank after `#` | 13 one | 100%: default | as written | not decided | 4,882 of 5,208 (93.7%) |
| blank lines | 13 at most one | 100%: default | one, two | yes | 6,389 of 6,391 (100%) |
| trailing blanks in the file | 13 none | 100%: default | none | yes | 295 of 317 (93.1%) |
| final newline | 13 yes | 100%: default | yes | yes | 316 of 317 (99.7%) |

`agreement` on the held-out half: 28,114 of 34,827 occurrences (80.7%) pooled over the decided rows; the mean of the rows' project means is 82.4%. Three rows are below 80% on their own and one of them is above the pooled number only because of the others: conditional body indent (15.3%), continuation indent of a rule or directive (4.0%) and the blanks around an assignment operator (66.0%; the other form is `A=b`, 28% of the held-out assignments, redis, node and containerd style, which the option `space_around_assignment` already covers). The rows of the corpus say: do not indent the body of a conditional by default (8 of 11 projects write none, and 3 more use a tab per level), and keep the indent of a continued line as written. A row left "as written" is not counted in the agreement.

Stable (`scripts/parity.sh stability makefile`, debug build with the safety check): 1,008 of 1,021 files (98.7%); the 13 that are not (12 NMake files of vim and `Make_ami.mak`, which is not make syntax) fail to parse, none changes on a second format or is refused.

## Meaning oracle
The only reference for what a Makefile means is GNU make, and it cannot be run on downloaded files: `make -pn` (print the database without running recipes) still expands `$(shell ...)`, evaluates `include` and `$(eval ...)`, and the host has no sandbox (no user namespaces, no container). It is not used. What is checked safely: `evidence_makefile.py` compares the text of every statement before and after the format with blanks removed (a change of any non-blank character fails; this is the safety check of the formatter again, independent of its parse) and flags the strip of a trailing blank on an assignment value. A check with the real make would need a sandbox host first.

## Left out
Choices of taste (the defaults follow the votes above): conditional body indent as written (option `conditional_body_indent`); blanks around an assignment (default one each side, the option exists); the alignment of assignments and the indent of continued lines as written (option `continued_line_indent`); the trailing blank of a value is kept (it is part of the value; mbake drops it from some lines). mbake's `.PHONY` regrouping and its recipe formatting are rewrites, not layout, and are not part of the plan. Lint is a separate leg (checkmake).

## Status
The format leg is registered with mbake as its tool (`xtask/full/support.yaml`, `scripts/parity_tools.json`, `docs/ai/parity/mbake.yaml`): target = the 1,007 files both tools read, agreement 88.2% (goal 85%), `second-format` 0. The `makefile/mbake` preset sets `spaced_assignments`, `continued_line_indent`, `keep_comment_gap`, `indent_style` and `max_blank_lines`; the remaining causes are above.
