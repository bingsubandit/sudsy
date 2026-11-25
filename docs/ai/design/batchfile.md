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
| indent characters | 10 spaces, 5 tab | spaces, option for tab | 74.0% |
| block body indent | 6 two, 4 four, 4 tab, 2 none | as written | n/a |
| closing paren indent | 16 same as opener | same as opener | 99.6% |
| label column | 15 column 0 | column 0 | 99.9% |
| command word case | 15 lower | lower | 94.0% |
| blank before `(` | 16 one blank | one blank | 98.3% |
| else line | 13 `) else (` | `) else (` | 97.5% |
| blank after REM | 14 blank | blank | 100% |
| blank after `::` | 9 blank, 1 none | blank | 73.1% |
| continued line indent | 8 none | none | 76.6% |
| blanks around `==`, redirections, case per file | no 60% majority | as written | n/a |

Pooled agreement 93.7% of occurrences (dominated by trailing blanks), mean of the rows 90.5%. Rows under 80% on their own: indent characters, blank after `::`, continued line indent. A formatter changes only blanks, line ends and case; trailing blanks stay after `set x=y ` and `echo x ` where they are part of the value or text. The stable-on-second-format number and the 30-file harm reading need a formatter and are not measured.

## Parser candidates
Run over the corpus (547 files, 3.02 MB), tree-sitter 0.27.2, parsers compiled from their shipped `parser.c` at -O2, whole files from memory, warm, best of 5, AWS Neoverse-V2 aarch64.

| Candidate | Files without an error node | Error nodes | Speed | parser.c object (text) | Notes |
|---|---|---|---|---|---|
| wharflab/tree-sitter-batch 0.11.1 (MIT) | 434 (79.3%) | 5,905 | 11.8 MB/s (746 KB file in 77.9 ms) | 169 KB | 53 node types, no external scanner; 15 stars, 90k crates.io downloads, Helix uses it |
| davidevofficial | 5 | 30,536 | 4.3 MB/s | 139 KB | rejected |
| imDMG/tree-sitter-bat | 19 | 65,339 | 5.9 MB/s | 23 KB | rejected |
| SklorGalore | not built | | | | ships no generated `parser.c` |
| oak-cmd 0.0.11 | not measured | | | | stub: one `Text` token per character |
| hand-written line parser | estimate only | | | | 1,200 to 1,800 lines of Rust in the language crate (line and continuation splitter, quote, caret and percent masking, block nesting, IF/FOR/SET heads), more than half of the 3,000-line budget |

wharflab, error recovery: deleting one `)` in 258 files, the error bytes are 14.6% of the file at the median and 100% at p75 (101 of 258 mutants produce no error node, as the file stays valid); a 100,000-deep `if (` nest parses with no error and no stack problem (300 ms).

wharflab, 113 files with an error node (21 of 31 projects; 6 of the 59 files without a final newline): by first error, 12 gradlew `if not ""=="%GRADLE_EXIT_CONSOLE%" exit`, 16 polyglots with a Unix shell or PowerShell part, 8 `for %%# in (%*)`, 8 `call .\path`, 5 hyphenated names (`set hadoop-command=`), 3 `call :%VAR%`, 3 `%ProgramFiles(x86)%` in a `for /f` command, 58 others (text with parentheses after `echo.`, `[[Placeholder]]` templates, `exit \b`). A file without a final newline can wrap its last command in an error node. 39 of the 113 have an enumerable grammar cause.

wharflab, wrong trees: 30 random error-free files, 373 lines starting with `set`, `if`, `for`, `goto`, `call`, `exit`, `setlocal`, `endlocal`, `rem`, `::` or a label: 1 wrong kind (`call "..." ^` read as `cond_exec`).

wharflab, constructs (30 probes, one per form): no error node for `^` escapes and `^` line continuation, `%var%`, `%~dp0`, `%*`, substring and replace forms, `!var!`, labels with GOTO and CALL, `for` plain, `/f` with options, back-quote and single-quote commands, `/l`, `/d`, `/r`, `if` with `else`, `else if` chains and one-line `else`, `if /i`, `errorlevel`, `defined`, `rem`, `::`, trailing `rem` after `&`, redirections, `&`, `&&`, `||`, pipes, `set /a`, `set /p`, quoted `set`, `echo.` and `echo(`, nested blocks, `exit /b`, `start`, `pushd`. Two fail: a final line with no newline (`echo a`) and the `%=empty=%` macro inside a quoted `set`.

Recommendation: adopt wharflab's grammar as the source of `grammars/batch` and fix its enumerated causes in our copy (the grammar is MIT, 169 KB object, pure grammar, 11.8 MB/s, 79.3% clean now, about 86% with the 39 enumerable causes). It is the only measured candidate over 20% clean, and the hand-written parser would spend over half of the language budget on a splitter that tree-sitter already provides. The choice is the user's. Open maintenance risk: the commits visible in the last months are mostly bot updates.

## Open
- Taste class in or out of the lint target (above).
- The three format rows under 80% become options with the written default.
