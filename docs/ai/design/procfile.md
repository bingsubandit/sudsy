# Design: Procfile evidence

Measurements before any Procfile product code (method: [../development/no-reference-tool.md](../development/no-reference-tool.md), [../architecture/adding-a-language.md](../architecture/adding-a-language.md)). Scope: process files named `Procfile`, `Procfile.dev`, `Procfile.local` (`name: command` per line; Heroku, foreman, honcho, overmind, hivemind, dokku, herokuish). Scripts: `scripts/evidence_procfile*.py|sh`, `scripts/evidence_procfile_usage.sh`, `scripts/evidence_procfile_search.sh`. Looked up 2026-11-26. Machine: AWS Neoverse-V2 aarch64, 8 cores. Heroku article fetched 2026-11-26 (`https://devcenter.heroku.com/articles/procfile`, "Procfile format" section).

## Corpus
839 files in 732 projects (66 KB), pinned in `tests/conformance/fetch-procfile-corpus.sh`: 813 `Procfile`, 26 `Procfile.dev`, no `Procfile.local`. Two batches:
- Batch 1, 45 projects pinned by commit, listed before any result (Heroku sample apps and buildpacks with their test fixtures, foreman, honcho, dokku, herokuish, overmind, hivemind, Rails, Django and Node applications). Three had no default-branch head and were dropped before any fetch. 127 files.
- Batch 2, the first 1,000 items of the GitHub code search `filename:Procfile` (10 pages), pinned by blob sha in `tests/conformance/procfile-search.tsv` (repository, path, sha). The search matches fuzzy names too (`procfile.py`, `Procfile.txt`, `docs/procfile.rst`); those are left out. 712 files, mostly one Procfile per repository.

Batch 2 was added after a first count of batch 1 showed 140 files in 8 projects with 3 or more files: the first corpus did not reach the 300-file, 20-project bar, and a Procfile is one short file per repository, so a project threshold of 3 files (as in the Gradle step) was dropped. This is recorded because batch 2 was chosen after batch 1's counts were seen; its selection is the search order, not the file content. `scripts/evidence_procfile.py` caps a project at 60 files (none reached it).

Split by project, seed 20261204: 366 deciding, 366 held out (`python3 -I scripts/evidence.py procfile list`). 362 deciding projects have a process line; the held-out half has 585 process lines.

## Tools (usage evidence)
| Tool | Evidence | Verdict |
|---|---|---|
| dokku/procfile-util (Go, Dokku org, apt package) | 9 stars, pushed 2026-11-23; `check` (validate), `list`, `show`, `expand`; `check --strict` (DNS-label process names); `procfile-util` in 105 code search hits, 3 of them in `filename:Procfile` files; no formatter | partial reference for lint (its validator rules, with the strict option); not primary by usage. Lead decides |
| foreman (6,165 stars), honcho (1,718), overmind (3,766), hivemind (1,162) | process runners; each reads a Procfile with its own regex (sources in the Lint section) | runtime sources for "line ignored" and "duplicate name", not format references |
| Heroku Procfile article | official specification of the format: `<process type>: <command>`, the process type is "an alphanumeric name", `web` and `release` special | specification |
| Go `chrismytton/procfile` (14 stars, 2020), Rust `jzeuzs/procfile` (4 stars, pushed 2026-11-25), Python `smartmob-project/procfile` (1 star); npm `procfile` (261 downloads a month) and `procfile-parser` (21), PyPI `procfile` (not read) | parse libraries | parse only, no layout; not references |
| zorab47/procfile.vim (6 stars, 2025) | Vim syntax file | editor only |
| paketo-buildpacks/procfile (23 stars), heroku/buildpacks-procfile (10) | buildpacks that turn a Procfile into process types | not lint or format |
| formatter for Procfile | repository search "procfile formatter", "procfile lint", "tree-sitter procfile": none | none found |
| procfile-lint on npm and PyPI | no package | none found |

Code search for the term "procfile linter" returns 324,608 hits; it matches prose in every repository and is not usage evidence.

## Format
No formatter exists. The decision table (`python3 -I scripts/evidence.py procfile table`; one vote per project over the 366 deciding projects; agreement is the share of the held-out occurrences with the chosen form):

| Row | Votes (deciding projects) | Rule | Chosen | Held-out agreement |
|---|---|---|---|---|
| blank after `:` | 357 one, 5 several (362) | default | one | 98.5% (576 of 585) |
| blank before `:` | 362 none | default | none | 100.0% (585 of 585) |
| blank after `#` of a comment | 12 one (12) | default | one | 77.7% (185 of 238) |
| blank lines in a row | 7 one (7) | default | one | 86.8% (33 of 38) |
| blank lines at the start of a file | 366 none | default | none | 99.8% (422 of 423) |
| blank lines at the end of a file | 359 none, 7 present (366) | default | none | 96.7% (409 of 423) |
| trailing blanks of a line | 357 none, 9 present (366) | default | none | 98.9% (862 of 872) |
| final newline | 218 yes, 148 no (366) | as written (59.6% yes, below 60%) | as written | n/a (277 of 423 held-out files end with a newline, 65.5%) |
| line endings | 358 LF, 8 CRLF (366) | default | LF | 97.2% (411 of 423) |
| `\` continuation of a process line | 362 single (362) | default | single | 100.0% (585 of 585) |
| indent of a process line | 362 none (362) | default | none | 99.5% (582 of 585) |
| process name case | 358 lower, 4 not lower (362) | not a layout choice: a process name is an identifier | as written | n/a (585 lower) |

Pooled held-out agreement over the decided rows: 4,650 of 4,757 = 97.8% (mean of the rows' project means 96.6%). The comment row is the one under 80%. The blank after `:` counts the lines with a command: 576 one space in the held-out half, 4 none, 4 several, 1 tab.

Not done: stability (`scripts/parity.sh stability`, no formatter), harm sample (no formatter), meaning oracle. The runtime parsers of foreman and honcho give a meaning check for the form of a process line (the process name and command they read); not run.

## Lint
Checklist (`python3 -I scripts/evidence_procfile_checks.py`; occurrences / files / projects; held-out occurrences). The sources are the parsers of the runners (`foreman lib/foreman/procfile.rb`: `/^([A-Za-z0-9_-]+):\s*(.+)$/`, other lines ignored; `honcho/environ.py`: the same regex and `assert` on duplicate names; `overmind start/procfile.go`: `/^([\w-]+):\s+(.+)$/`, `Process names must be unique` fatal, `No entries were found in Procfile` fatal) and the Heroku article ("`<process type>` is an alphanumeric name").

| Entry | Source | Occurrences | Files | Projects | Held-out |
|---|---|---|---|---|---|
| name outside `[A-Za-z0-9_-]`: the line is ignored | foreman, honcho; Heroku article | 9 | 9 | 9 | 2 |
| blank before the name: the line is ignored | the three `^` anchors | 3 | 2 | 2 | 3 |
| no blank after the colon: overmind ignores the line | overmind `:\s+` | 4 | 4 | 4 | 4 |
| empty command (`name:` only): ignored | foreman, overmind | 0 | 0 | 0 | 0 |
| backslash at the end of a line: the next line is ignored | the three parsers (no continuation rule) | 0 | 0 | 0 | 0 |
| duplicate process name: overmind fatal, honcho assert | overmind, honcho | 9 | 7 | 7 | 9 |
| no process line in the file: foreman and overmind fail | foreman EmptyFileError, overmind | 11 | 11 | 11 | 4 |
| a line with no colon (not a comment): ignored | the three parsers | 20 | 10 | 10 | 16 |
| trailing blanks of a line | style | 19 | 18 | 18 | 10 |
| CRLF line endings | style: foreman and honcho accept (`gsub("\r\n")`, `splitlines`) | 20 | 20 | 20 | 12 |
| no final newline | style | 295 | 295 | 283 | 146 |

Yield: 9 entries with occurrences, 2 with none (empty command, backslash), both kept because the parsers' source gives them. Comment lines (302 in 29 files) are not an error: all three skip them. The corpus has no `//` comment line (procfile-util accepts them, see Tools). 748 of 839 files have a `web:` line.

Checklist review by the lead and the precision sample (50 findings judged by a reader that did not write the rule) are not run. The checklist entries need the bad and good examples written out before the lead review; the bad example of each is the line above.

## Parser
Procfile is line-based: a process line is `name`, optional blanks, `:`, optional blanks, `command` (the command is the rest of the line, a shell command; `#` inside it is not a comment unless a runner says so). No tree-sitter grammar for Procfile was found: the repository search for `procfile in:name` gives parsers in Go, Rust, Python and npm, and the Vim syntax file, not grammars. The shared shape `key-value-layout` (`crates/core-format/src/shapes/key_value.rs`, reads `tag:key-value`) already has the decided form as its default (`space_before_separator: false`, `space_after_separator: true`), but it reads a grammar's tag and no Procfile grammar exists. The other line-based languages parse with tree-sitter through `core-treesitter` (INI, .gitignore, EditorConfig). A hand-written line parser is the other option (`core-config/src/editorconfig.rs` is one). The choice is the user's: a tiny tree-sitter grammar (name, colon, command, comment lines) or a hand-written line parser in a shared crate. Not run: no parser was built or timed.

## Not verified
Whether the corpus is typical of production Procfiles (batch 2 is the search order of code search, one file per repository; batch 1 is the chosen list); the 59.6% of final newlines (a boundary value, read as "as written"); the CRLF effect on overmind (`(.+)$` keeps a trailing `\r` in the command by the source, not run); whether Heroku's own buildpacks accept `name:command` without a blank (not read); the rule set of procfile-util beyond its README and `docs/procfile-format.md` (its `check` output not run on the corpus); the checklist review, precision sample, harm sample, stability and meaning oracle; the npm and PyPI packages named in Tools (not read).
