# Design: Batchfile evidence

Measurements before any Batchfile product code (method: [../development/no-reference-tool.md](../development/no-reference-tool.md), [../architecture/adding-a-language.md](../architecture/adding-a-language.md)). Rule-by-rule tables: [batchfile-rules.md](batchfile-rules.md). Looked up 2026-11-25.

## Sources
| Tool | Evidence | Verdict |
|---|---|---|
| Blinter 1.1.74 (BatchLang/Blinter, AGPL-3.0, Python) | 54 stars, 3 contributors, pushed 2026-11-16, 234 PyPI releases, PyPI 20,445 downloads a month, VS Code extension `14ag.blinter` 6,335 installs; 4 `.pre-commit-config` and 48 workflow hits on GitHub; in no nvim-lint, conform.nvim or mega-linter list | primary lint tool; no maintained competitor |
| BatchCraft (JetBrains) | 14.6k downloads; inspections for labels, `!VAR!` without delayed expansion, `%VAR%` set and read in one block, `set X = y`, `::` in blocks, unquoted comparisons | editor only, not a reference |
| bat-formatter (`wwnao.bat-formatter`) | 15.6k installs, repo 2 stars, last push 2025-01-30; regexes that collapse blank lines and indent blocks by 2 | a toy, not a reference |
| rech-editor-batch | 57.9k installs; language support, no formatter found | none |

No maintained formatter exists: the format leg has no reference tool and follows the decision table below. cmd.exe does not run here and downloads are never run, so there is no meaning oracle for format.

## Corpus
547 files (3.02 MB) of 31 projects, pinned by commit in `tests/conformance/fetch-batchfile-corpus.sh`, chosen before any result (the projects of well known repositories with `.bat` or `.cmd` files; `vendor`, `node_modules`, `third_party`, `testdata` paths are left out; `scripts/evidence_batchfile.py` caps a project at 40 files and skips the LLVM OpenMP debugger `.cmd` files, which are not Batch). Cassandra and Electron have no such files. Tarballs carry the line endings stored in git, not those of a Windows checkout.

Split: the 31 projects are shuffled with seed 20261201; the first 16 decide (243 files), 15 are held out (304 files). Deciding: CMake, cmder, emscripten, git-for-windows, hadoop, intellij-community, jdk, llvm-project, maven, node, opencv, php-src, protobuf, pytorch, ruby, tensorflow. Held out: Microsoft-Activation-Scripts, choco, cpython, dotnet-aspnetcore, dotnet-runtime, elasticsearch, gradle, kafka, rust, spark, tomcat, tron, vim, windows-terminal, wireshark.

## Lint target
Blinter with `--no-config` (every rule is on; it turns no opinion off by default) over the 547 files: 26,692 findings of 144 rules in 546 files, no tool failure. Per rule, 3 to 4 findings over distinct projects were read for the 62 most frequent rules (94% of the findings); the other rules are judged by name and marked in the table. A seeded sample of 30 findings over the whole corpus: 16 true as stated, 11 wrong (tool bugs), 3 undecidable (undefined variables).

| Class | Rules | Findings | Share | Weighted by project |
|---|---|---|---|---|
| keep: right, decidable from the file | 56 | 10,353 | 38.8% | 24.9% |
| taste: true as stated, no hazard (a preference) | 55 | 6,500 | 24.4% | 27.2% |
| bug: sampled findings wrong or benign | 21 | 6,361 | 23.8% | 29.6% |
| env: needs the environment or cmd.exe | 9 | 3,195 | 12.0% | 13.9% |
| artifact: the corpus makes it fire | 3 | 283 | 1.1% | 4.4% |

Not counted:
- bug (6,361): SEC013, S014, SEC002, SEC014, W004, E001, W035, P016, E042, S001, W028, W011, W053, W051, W033, P022, P012, W067, W057, SEC011, E012. Examples: S014 counts redirections and quoted words as parameters; W035 flags `FOR /F` without `delims`, which is the default; W051 reads `%f%648` as `%648`.
- env (3,195): E006 undefined variable (2,209; the variable may come from the caller or the environment), E043 `::` in a block (harm needs cmd.exe), SEC005, SEC020, W007, W009, W010, W027, W029 (privileges, Windows versions, files in use).
- artifact (283): E018, W018, W019 need LF endings of the checkout; the tarballs lose git's conversion.
- Opinions the tool turns off by default: none (0 findings).

The target is the rest: keep and taste, 16,853 findings of 111 rules in 543 files (needs the line 11,375, the file 1,623, the control flow 3,855). Keep alone (the core, the real defects and style rules) is 10,353 findings of 56 rules in 323 files. The taste class (S019 magic numbers, W002 and W003 and W025 missing ERRORLEVEL checks, W001 no EXIT, W023 and W039 nested FOR, S024, S007, S008, SEC004) is a decision of the user: keeping it makes S019 and the ERRORLEVEL checks part of the 85%.

## Plan to 85% (lint)
Cumulative share of the target, rules in order of findings (full table in [batchfile-rules.md](batchfile-rules.md)):

| Rank | Rule | Share | Weighted by project | Held out | Family |
|---|---|---|---|---|---|
| 1 | S020 line length | 37.1% | 16.8% | 40.8% | `max-line-length` (exists) |
| 5 | W005 unquoted IF operand | 55.1% | 24.4% | 60.6% | `pattern-rules` |
| 10 | W039 nested FOR | 66.7% | 41.4% | 71.1% | `pattern-rules` (nested loop relation) |
| 15 | W025 missing error handling | 76.3% | 59.0% | 79.5% | missing: command not followed by a check |
| 20 | S003 command case | 83.7% | 71.3% | 85.6% | `token-case-lint` |
| 22 | W021 IF without quotes | 85.5% | 73.1% | 87.6% | `pattern-rules` |
| 40 | W066 `2>&1` order | 93.5% | 85.0% | 94.7% | `pattern-rules` |
| 111 | all | 100% | 100% | 100% | |

Raw 85% is reached at rank 22, held out at rank 20, weighted by project at rank 40 (the huge Microsoft-Activation-Scripts files carry S020). Core only: S020 60.3%, S015 65.5%, W005 70.2%, S029 73.8%, S027 77.0%, S004 80.0%, W068 82.6%, S028 85.1% (rank 8).

Shared families that exist: `max-line-length`, `no-trailing-whitespace` (S004), `blank-line-before-family` (S027), `token-case-lint` (S003), `scope-rules` (S010, W013, E002, W040), `unreachable-code` (E008), `format-diff-family` (S012), `documented-command`, `file-name`, `declared-name-pattern`, `max`. Most E rules and W005, W021, S029, S028, W052 are `pattern-rules` over grammar nodes. Missing shared mechanisms, in order of findings they carry: command not followed by a check (W002, W003, W025: 2,200), number literal outside an allowed set (S019: 1,322), one form per file (S015: GOTO with or without colon), expansion time in a block (W068, W071; Makefile `timestamp-expanded` is the same idea), file header comment (S013), paired commands (W061 PUSHD without POPD).

## Format (decision table)
No reference tool, so one vote per project over the deciding half and the agreement on the held-out half (`scripts/evidence_batchfile.py`; `python3 -I scripts/evidence.py batchfile table`). Rule: 80% of projects or more is the default, 60 to 80% default plus option, below 60% as written.

| Row | Deciding votes | Result | Held-out agreement |
|---|---|---|---|
| line endings | 8 CRLF, 8 LF | as written | n/a |
| final newline | 16 yes | yes | 86.2% |
| trailing blanks | 16 none | none | 99.7% |
| blank lines | 16 one | collapse to 1 | 96.7% |
| indent characters | 10 spaces, 5 tab | as written, option `indent_conversion` (spaces or tabs) | 74.0% |
| block body indent | 6 two, 4 four, 4 tab, 2 none | as written | n/a |
| closing paren indent | 16 same as opener | same as opener | 99.6% |
| label column | 15 column 0 | column 0 outside blocks (a label in a block stays) | 99.9% |
| command word case | 15 lower | lower: internal commands and the keywords `if else for in do rem`; programs, switches, `EQU`, labels, variables and arguments keep theirs | 94.0% |
| blank before `(` | 16 one blank | one blank before the `(` of a block | 98.3% |
| else line | 13 `) else (` | `) else (` | 97.5% |
| blank after REM | 14 blank | at least one blank (more stay) | 100% |
| blank after `::` | 9 blank, 1 none | at least one blank, option `space_after_colons` | 73.1% |
| continued line indent | 8 none | as written, option `continued_indent: none` | 76.6% |
| blanks around `==`, redirections, case per file | no 60% majority | as written | n/a |

Pooled agreement of the rows the formatter writes (held-out occurrences already in the form it writes): 96.9% (99,162 of 102,323; the three rows under 80% are options whose default is the written form, so they are out of the pool), mean of the rows 93.4%. Row under 80% on its own: blank after `::` (73.1%). A formatter changes only blanks, line ends and case; trailing blanks stay after `set x=y ` and `echo x ` where they are part of the value or text. `python3 -I scripts/evidence.py batchfile <sudsy> <work>` re-measures it with the formatter: 712 of 719 corpus files format (the 7 others are the files below), none changes meaning or is unstable on a second format, 30 changed files read for harm (the two harms found, a collapse of the blanks inside comment text and a label moved out of a block, are fixed: comment blanks stay and only a label outside a block goes to column 0).

## Parser
Hand-written, in `crates/lang-batch` on `core_syntax::parse` (no tree-sitter: the choice of the step prompt over the measurement of the candidates, which is in git history: wharflab/tree-sitter-batch 79.3% of 547 files without an error node at 11.8 MB/s, the others under 4%). The lexer cannot be context free, because cmd reads a line by the command it starts: `rem` and `::` take the rest of the line, `echo`, `set` (plain), `title` and `prompt` take text whose blanks are content, `if`, `for` and `else` are keywords only at the start of a command (`else` after `)`), and `)` ends a word only inside a block or the list of a `FOR`.

- `lexer.rs` is one pass over the bytes with the state `want_command`, `text`, the block depth, the parentheses opened in the arguments of the command and whether the command word is a variable. Tokens tile the source (blanks, line breaks and `^` plus line break are trivia). Facts that are probes of cmd rather than guesses are in the module comment; no cmd.exe was available, so each is read from documentation and the corpus: `rem` and `::` swallow `^` and `)`, `^` plus a blank escapes the blank, `cd..`, `echo.`, `dir/b` and `goto:eof` split the command word at the delimiter, a `(` in an argument is balanced by its own `)` (so `echo x (%v%)` in a block stays in the block), a variable that stands for a command (`%t% (`) opens a block, `>&2` ahead of `(` belongs to the block, a label inside a block is a label (labels at column 0 in a block are valid, so a missing `)` is one `unclosed-block` error and the block runs to the end of the file).
- `parser.rs` is a linear pass over the tokens: a file is lines, a line a `Sequence` of statements joined by `& && || |`, a block holds lines, `IF`/`ELSE`/`FOR` take the rest of their line or a block. Nesting is bounded by `MAX_DEPTH` (a deeper group is one `ERROR` node); 100,000 nested `(`, `if` and `for` parse with no stack problem.
- Result on the 719 files of the corpus folder (the 547 of the evidence list plus 172 more, among them 35 `.c.cmd` test files of llvm-project): every file prints back byte for byte (OEM bytes, mixed CRLF and LF, no final newline included). 7 files (1.0%) have an error node: 6 are polyglots with PowerShell or C# text in the file (`Change_Windows_Edition`, `Check_Activation_Status`, `HWID_Activation`, `MAS_AIO`, `Troubleshoot`, `TSforge_Activation` of Microsoft-Activation-Scripts) and 1 is `ansi-color.cmd` of Windows Terminal, whose lines are a macro (`SET @strlen=FOR ... (%\n%` over many lines, run by a jump, not in sequence). No file of valid sequential cmd syntax has an error node. Mutants (the last `)` of 258 clean files deleted): 172 have an error; the other 86 raise none (not classified: the `)` was probably text, in a comment or quoted). Speed (release, one thread, whole files from memory, 10 passes, Neoverse-V2 arm64 agent host, the 719 files, 3.27 MB): 44.7 MB/s against 11.8 MB/s of the tree-sitter candidate (measured on 547 files, same host).

## Format: mapping table
Every row of the decision table, with the kit entry it reads:

| Row | Entry |
|---|---|
| line endings, final newline, blank lines | `token-line-stream` (`end_of_line`, `final_newline`, `max_blank_lines`) |
| trailing blanks | `token-line-stream` (the printer trims; text that holds its blanks is one token printed verbatim) |
| indent characters | `indent-conversion` (new, `option:indent_conversion`, `option:indent_size`) |
| block body indent | `token-line-stream` with `source-indent`: every line keeps its indent |
| closing parenthesis | `token-line-stream` `align-closers` (new key of `format-rows`) |
| label column | `token-line-stream` `flush` (new key of `format-rows`, outside brackets only) |
| command word case | `token-case` (`[case]` rows) |
| blank before `(`, `) else (` | `token-spacing-rows` (`[space]` rows) |
| blank after REM | `token-spacing-rows` (`[at_least_one]` row) |
| blank after `::` | `token-line-stream` `option-blanks` (new key of `format-rows`: at least one blank between two kinds while a boolean option is on) |
| continued line indent | `indent-conversion` (`option:continued_indent`) |

`flush`, `align-closers` and `option-blanks` are plain `lang.yaml` kind sets read by one shared mechanism each (`core-format/stream.rs`); they become `format.rules` rows when a second language needs them. Search terms for "new": `grep -rn "flush\|align\|closer" crates/core-format/src` and the kit index words "indent", "tab", "column 0", "closing".

## Open
- Taste class in or out of the lint target (above).
- `echo OFF` keeps the case of its argument (`@echo OFF`): arguments keep their case, and `off` has no row of its own.
- No cmd.exe was available, so the cmd rules of the lexer are unverified against it; they are checked against the corpus only.
