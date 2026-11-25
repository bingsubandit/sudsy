# Batchfile (.bat / .cmd): measurement

Measurement only: nothing built, nothing landed. Machine: AWS Neoverse-V2, 8 cores, aarch64 Linux, rustc 1.99.0, gcc 13.3, release builds, 2026-11-25. Harness (outside the repo, in the slot): `/scratch/slots/sudsy/8/meas` (copies in `.cargo/tmp/meas-main.rs`, `meas-hand.rs`, `batch-snips.txt`). Fetch script `.cargo/tmp/batch-fetch.sh` + pins `.cargo/tmp/pins.txt`.

## Headline
- Corpus: 62 projects, 1,406 unique non-empty files, 3.74 MB, 93,472 lines. Easily past the 300 files / 20 projects bar.
- The queue says "lint: none found". Wrong: **Blinter** exists (Python, AGPL-3.0, 193 rules, 20,445 PyPI downloads last month, 54 stars, pushed 2026-11-16). The queue also says the format leg has no tool: true in the sense of a real formatter, but a basic one exists (`wwnao.bat-formatter`, 15,635 installs, blank lines and indent only).
- Parser: the best tree-sitter grammar (wharflab, MIT) leaves **28.6% of the corpus files with ERROR/MISSING** nodes (44.3% if a missing final newline is not repaired); the other two grammars fail on 96-99% of files. A hand-written lossless lexer is 34x faster (458 vs 13.3 MB/s) and tiles all 1,406 files.
- Score: 4.58~ becomes 4.62 (tree-sitter) or 4.22 (hand-written) for "both"; the lint-only plan falls from 3.48~ to 2.25-2.54. The format leg alone is cheaper than estimated, but a format leg for Batchfile is thin (section 3). My opinion in section 5: not the best next language.

## 1. Corpus
- 62 repositories (pins in `.cargo/tmp/pins.txt`: `dir url sha`), picked before any result was seen from: large permissive projects that ship `.bat`/`.cmd` (Apache, .NET, Python, Node, LLVM, Mono, IntelliJ, Hadoop, Kafka, ...) plus hand-written script collections (`npocmaka/batch.scripts`, `bmrf/tron`, `pal1000/mesa-dist-win`, WinPE builders, launchers). Dropped before measuring: repositories with a non-permissive licence (elasticsearch ELv2/SSPL/AGPL, vim, openjdk GPL) and those with 0 files left (cassandra, electron, skia, ninja, openssl, coreclr, corefx, onnxruntime failed to fetch).
- Licences are the projects' published ones (Apache-2.0, MIT, BSD-3, PSF, BSL, curl, Tcl); a root-LICENSE scan agreed except where the file is not in the root (intellij, mono, kotlin, roslyn, diligent, driver-samples, sqlite-tcl: all published as Apache-2.0 or MIT). Not verified file by file.
- Fetch: `CORPUS=<dir> bash .cargo/tmp/batch-fetch.sh .cargo/tmp/pins.txt`: per project `git init`, partial clone (`--filter=blob:none --depth 1` of the pinned SHA) with sparse checkout of `*.bat *.cmd` (and case variants) plus LICENSE/COPYING. Whole corpus about 60 s. The corpus lives in a gitignored folder (like `tests/conformance/corpus`); the working copy is `/scratch/slots/sudsy/8/corpus-batch` (296 MB with .git, files alone 3.7 MB).
- Sizes: 1,514 files found, 107 content duplicates (vendored copies; aspnetcore 40, intellij 13, llvm 12) and 1 empty dropped: **1,406 files, 3,736,906 bytes, 93,472 lines**. Per project from 1 to 196 files (largest: batch-scripts 196, aspnetcore 79, sillytavern-launcher 108, mesa-dist-win 96, winpe-builder 83, llvm 59, intellij 57); largest by bytes: sillytavern-launcher 595 KB, batch-scripts 458 KB, tairiku-windows 340 KB, tron 302 KB.
- Line endings: CRLF 620 files, LF 784, mixed 2. UTF-8 BOM 8 files. **Not valid UTF-8: 25 files (1.8%)** (legacy code pages); no final newline: 262 files (18.6%).
- Split for the no-reference-tool rule: seed 20261125, `random.shuffle` of the sorted project names, first 31 deciding (757 files), last 31 held out (649 files). Lists in `.cargo/tmp/batch-files.txt` (`D|H`, project, path). Files in the corpus that are not really batch (polyglot with REXX, JScript, C#, shell, PowerShell) stay in: that is what real `.bat` files are.

Construct census (share of the 1,406 files that contain it, regex on the text): `%var%` 72.1%, `%~` modifiers 57.8%, echo 76.5%, REM 49.7%, `::` 19.1%, labels 36.8%, goto/call 42.3%, `if ... (` block 37.7%, `) else (` 20.4%, if exist/defined 35.6%, if errorlevel 8.3%, `for /f` 22.4% (plain for 11.2%, /l 1.6%, /d or /r 3.3%), `set "a=b"` 23.8%, set /a 6.5%, set /p 7.3%, redirection 33.1% (`2>&1` 8.0%), pipe 19.2%, `&&`/`||` 16.0%, `%x:a=b%` 15.6%, `!var!` 10.0%, caret escapes in a line 12.1%, caret line continuation **5.6%**, tab indentation 13.4%, lowercase keywords at line start 80.7% / UPPERCASE 24.6% (files can have both).

## 2. Parser candidates

### Candidates found
| Candidate | Licence | Last change | Coverage / state |
|---|---|---|---|
| `wharflab/tree-sitter-batch` (crate `tree-sitter-batch` 0.11.1, 90,203 downloads) | MIT | default branch: last commit 2026-07-22, pushes to other branches to 2026-11-24; 76 commits since 2026-03-10 (37 by one human, 39 renovate bot), 19 tags, 15 stars | 295-line `grammar.js`; 53 named node kinds: if/else, for (/f /l /d /r), set (/a /p, quoted), goto, call, echo (free text), labels, REM and `::`, redirects with fds, `&`, `&&`, `||`, pipes, `%~dp0`, `%x:a=b%`, `!x!`, parenthesized blocks; 34 corpus test files; no external scanner; ABI 14 (pinned for Go consumers) |
| `davidevofficial/tree-sitter-batch` | MIT | 2025-02-21, 14 commits, 2 stars | 67-line grammar; node kinds `identifier`, `keyword`, `number`, `function_definition`: a highlighter, no statement structure |
| `imDMG/tree-sitter-bat` | MIT | 2026-03-13, 4 commits, 0 stars | 153-line grammar; `command`, `label`, `for_variable`, `redirect_operator`...; early |
| `SklorGalore/tree-sitter-batch` | none | 2026-04-13, 1 commit | 17-line stub, no generated parser: unusable |
| Rust crate | - | - | none on crates.io beyond the wharflab grammar (`tree-sitter-bat`, `batch-parser`, `cmd-parser`, `tree-sitter-dosbatch` do not exist; crates.io and GitHub searches for "batch/cmd parser" found no Rust parser) |
| Haskell `BYVoid/language-batch` | none | 2015 | parser and beautifier, abandoned: not a candidate |
| hand-written (lang-cmake pattern: lossless lexer on `core_syntax::parse::Lexer` + a flat statement parser, `blocks.rs`) | ours | - | prototype lexer measured below |

### Measured, same corpus (1,406 files, 3.74 MB), whole files from memory, warm, best of 5, no adapter
| Row | wharflab | davidev | imdmg | hand-written lexer (prototype) |
|---|---|---|---|---|
| Kind | C, generated, untyped tree, whitespace is extras | C, generated | C, generated | Rust, lossless tokens, no tree yet |
| Parse speed | 282 ms = **13.3 MB/s** | 804 ms = 4.6 MB/s | 673 ms = 5.6 MB/s | 8.2 ms = **458 MB/s** (lexing only; 605,232 tokens) |
| Files with ERROR/MISSING | **623 = 44.3%** as is; **402 = 28.6%** when a missing final newline is added (grammar needs `\n` at EOF: MISSING `program_token1`) | 1,395 = 99.2% | 1,354 = 96.3% | n/a: 2 files = 0.1% end with an unbalanced block depth in the prototype |
| By split | deciding 36.7% (26% with newline), held out 53.2% (31.7%) | 98.8% / 99.7% | 95.1% / 97.7% | - |
| Bytes in ERROR nodes | 7.9% (294 KB) | 47.1% | 19.5% | - |
| Lines touched by ERROR | 11.5% (10,893 of 94,878) | 61.8% | 47.4% | - |
| Projects with more than half their files in error | 17 of 62 (10 with the newline fix) | 62 | 61 | - |
| Probe snippets without ERROR (27 snippets, 8 cmd.exe constructs below) | **22 of 27** | 0 of 27 | 5 of 27 | - |
| Slowest file | 15.8 ms (sillytavern Launcher.bat, 270 KB) | 59 ms | 43 ms | - |
| Deep nesting (5,000 nested `if (`) | no stack problem, 14 ms, no error | not run | not run | iterative lexer |
| Tokens tile the source | not measured (the adapter rebuilds trivia, as for every tree-sitter language) | - | - | **yes on all 1,406 files** |
| Stripped binary added | object `parser.c`: **170 KB** text+data (parser.c 599 KB), gcc -O2; for scale: make 208 KB, go 227 KB, containerfile 103 KB, bash 1.22 MB | 140 KB | 24 KB | a few KB of Rust (lang-cmake's lexer and parser are 286 lines) |
| Build time | parser.c compiles in 0.33 s (-O2); tree-sitter runtime already linked | - | - | in the crate |
| Dependencies | `tree-sitter-language` only (+ `cc` at build) | - | - | none |
| Mutants (one bracket removed) | not measured | not measured | not measured | not measured |

Not measured: the tree-sitter adapter itself, memory, mutants, a full hand-written parser (only the lexer with block depth was prototyped, 174 lines, `.cargo/tmp/meas-hand.rs`).

Where wharflab fails on the corpus (checked by reading error lines, `errs-wharflab-nl.txt` in the slot):
- Polyglot content in `.bat` files (REXX in ant.cmd, JScript/C#/PowerShell/sh hybrids, `[DllImport]`, `}` and `#` lines): in at least 72 of the 393 error files every error line is non-batch text. These fail for any batch parser that does not know the polyglot trick; a hand-written parser would also have to mark them as opaque.
- Real gaps in the grammar (321 of the 393 error files have an error on a line that starts like a batch statement): caret-escaped `for` options (`for /f tokens^=1^,2^ eol^= %%a`, 104 error lines), loop variables like `%%#` / `%%@` (94), `if not ERRORLEVEL == 0 (` and `if errorlevel` compares (36), `set name-with-dash=` (56), `%x:"=%` substitutions (69 with `%~`/`%:`), `call .\path` (39), `goto %var%`, unquoted `if exist foo (echo (nested) text)`, `echo` text with special characters, `for ... in (`...`)` with backquotes (24). The snippet suite fails on: caret-escaped for options, `if not ERRORLEVEL == 0`, `echo "a & b" > "c d.txt"`, parens in an unquoted if body, `goto %var%`.
- A missing final newline (262 files, 18.6%) turns into a MISSING node: the provider would have to append one before parsing, which the lossless tree cannot do without a shim.

Coverage of cmd.exe syntax in wharflab (probe snippets): handles caret continuation, `%var%` with modifiers and substitutions, `!var!`, for plain /f /l /d /r and usebackq, if/else blocks and `) else if`, labels, goto/call, REM, `::`, redirections, echo variants, set variants, `&`/`&&`/`||`, nested blocks, `call set`, `start`, `cmd /v:on`, tabs/unicode. Fails on the five snippets above. The tree is loose: statements of an `echo` become `argument_value` runs, `if` bodies are `cmd` with `argument_list`; a formatter or linter would re-lex text slots (echo text, set values) itself.

Maintenance: wharflab is one human maintainer (37 commits, a company org, dependency bot for the rest), young (March 2026), active; fast churn (crate went 0.8 to 0.11.1 in 3 days of April 2026). Fixes land upstream; if we need them faster we carry a fork (CLAUDE.md names that cost for TOML). The grammar CI claims zero errors on its 18 `examples/` files; the corpus shows a much higher error rate on real code.

Hand-written: Batch is not context-free: what `(`, `)`, `^`, `%`, `!`, `"` mean depends on the command (echo text, set value, rem, for/in/do, the first word of the statement) and on delayed expansion. That is a lexer with command context, which is the shape of lang-cmake (`lexer.rs` 140 lines, `parser.rs` 146 lines for a far simpler language) plus the statement-level structure; lines of the language crate would be several hundred (my guess 700-1,200 for lexer + parser + block structure, to be measured in step 1). What stays opaque and exact (set values, echo text, for `in (...)` sets, quoted strings, comments) is exactly what a formatter and most rules must leave alone anyway. Errors: cmd.exe has almost no syntax errors (a script runs until the bad line), so a hand-written parser rarely refuses a file: the prototype found unbalanced block depth in 2 of 1,406 files; the tree-sitter route refuses 28.6% of real files (formatter refuses files with errors).

### Recommendation (the choice is yours)
Hand-written lexer and flat parser on the shared lexer base (like lang-cmake). Reasons: the best grammar fails on 28.6% of real files (7.9% of bytes), 34x slower than a lexer (13.3 vs 458 MB/s), a one-maintainer grammar that gets no tree for polyglots anyway, tokens and text slots have to be re-read by our own code regardless; the formatter needs byte-exact control of whitespace in values. Costs accepted: parser term 1.5 instead of 1.2 in the score and more language-crate lines (the 3,000-line budget is not at risk; hand-written Rust per language counts against the HARD RULE review, so lexing basics (blank runs, line continuations, quote scanning) must come from `core_syntax::parse`). Alternative if you prefer less code: tree-sitter wharflab pinned at `2aa22f0`, accepting that about 29% of real files are refused until upstream fixes the listed gaps (and a final-newline shim).

## 3. Format leg

### Tools (evidence of use)
| Tool | Evidence | Verdict |
|---|---|---|
| `wwnao.bat-formatter` (VS Code, MIT) | 15,635 installs, last update 2025-01-30, README: removes redundant blank lines, controls indent, no options | a partial editor formatter: a source for the comparison below ("partial tool") |
| `zhaofinger.batch-format` | 16,386 installs but it formats many files with the default formatter (its package.json has only a command `batchFormatFiles`): not a Batch formatter | not a tool |
| `BYVoid/language-batch` | Haskell beautifier, 2 stars, 2015 | dead |
| Prettier/conform/mega-linter/pre-commit/dprint/npm/PyPI/crates.io | nothing found for bat/cmd (searches: "batch formatter", "windows batch", "bat formatter", "cmd script formatter") | none |
| JetBrains "Batch Scripts Support" | exists; whether it formats not checked | unverified |
| Notepad++/VS Code built-in `vscode.bat` (367,800 installs) | highlighting, folding, snippets, no formatter | none |

So: no maintained tool; the leg is held to `docs/ai/development/no-reference-tool.md` (the queue's estimate "none found" holds for the format leg).

### What the rules would need (no-reference-tool.md sections 3 and 6)
- Corpus rule needs per-project votes on the deciding half. First numbers (`.cargo/tmp` script votes.py in the slot, one vote per project, winner per project; deciding half D, held-out H):

| Choice | D | H | Result under the 80/60 rule |
|---|---|---|---|
| Keyword case at statement start (`if/for/set/goto/echo/call/...`) | lower 27 of 31 = 87% | lower 25 of 31 = 81% | lower is the default (>= 80%) |
| `@echo off` (vs `echo off`) | 27 of 27 = 100% | 29 of 29 = 100% | default; but removing `@` is not a formatter's job: leave as written |
| `) else (` on one line | 21 of 23 = 91% | 19 of 19 = 100% | not a choice: cmd.exe requires it |
| No blank line after a label | 24 of 27 = 89% | 24 of 27 = 89% | default (as written, really) |
| Unquoted `set x=v` over `set "x=v"` | 26 of 29 = 90% | 23 of 31 = 74% | between 60 and 80 on H: as written; changing quoting is never safe |
| Indent inside blocks | 4 sp 10, 2 sp 9, tab 5, 3 sp 2 (38%) | 4 sp 12, 2 sp 10, tab 5 (43%) | below 60%: as written, or an option with "as written" default |
| Spaces around `==` in if | tight 17, spaced 11 (61%) | tight 14, spaced 13 (52%) | as written |
| Line ending | lf 16, crlf 15 | crlf 16, lf 15 | as written (50/50) |

  Result: of the layout choices, only keyword case reaches 80%. Everything else is "as written" or an option; the format leg would be: re-indent by block depth (width = detected or option), collapse blank runs, trailing whitespace where safe, keyword case, final newline, line endings per option. That is cheap (`layout: whitespace`, not `width`), and agreement 80% is nearly automatic because few constructs are decided; the real value of the leg is small. The score's `layout: width, parts: 1` (+0.2) should be `whitespace` (-0.3): wrapping is only safe at spaces between arguments outside quotes, echo text and set values, and only 5.6% of files use caret continuation at all. I would not build width wrapping.
- Stable: the formatter changes whitespace only, so `stable` 100% is easy once the parser tiles the source (the hand-written prototype does).
- Meaning oracle: **none runnable on Linux**; cmd.exe is Windows-only. wine's cmd is not an oracle. So the plan must say: the formatter changes only blanks, line ends and case where the manual says they carry no meaning (the list below), and a test checks that the token sequence modulo whitespace and keyword case is unchanged.

### Where whitespace and line structure carry meaning in cmd.exe (a formatter must not touch these)
1. `set x=value  ` keeps the trailing blanks as part of the value; `set x = v` makes variable `x ` with value ` v`; `set "x=v"  ` ignores what follows the closing quote. So trailing whitespace cannot be stripped on unquoted `set` lines.
2. `echo text`: everything after echo and one delimiter is output as is: leading and trailing blanks, and the blank before `>`: `echo a >f` writes `a ` and `echo a>f` writes `a`. A closing `)` after echo text includes the blank before it (`(echo a )` prints `a `): moving a `)` to its own line changes output.
3. Redirection handles: `cmd 2>nul` redirects stderr, `cmd 2 >nul` passes `2`; `echo 1>f` vs `echo 1 >f`.
4. `^` at the very end of a line continues the line; a blank after it breaks it. The first character of the continuation line is escaped (literal): a continuation line that starts with `&`, `|` or `)` does not do what it looks like. The leading blanks of the next line are part of set values, echo text and quoted strings: re-indenting a continuation changes the value.
5. `) else (` must stay on one line; `else` on a new line is a syntax error; `(` must be on the same line as `if ...` / `for ... do`. A formatter must never move `else` or `(` to a new line (Brace-on-own-line styles are impossible).
6. `rem` and `::`: `rem` ends at the newline and ignores `&`, `|`, `>` in its text; `::` is a label: a `::` comment as the last line of a block, or inside a block before `)`, is a syntax error. `rem` can replace `::` anywhere; `::` can not replace `rem` inside blocks. Comment text is never rewritten.
7. `&` / `&&` / `||` chains: splitting `a & b` into two lines changes meaning inside an `if`/`for` body without a block (`if x a & b`: `b` is part of the if body; the next line is not). Joining or splitting `&&` changes errorlevel behaviour.
8. Percent expansion happens when a line or a whole parenthesized block is parsed: moving a statement into or out of a block, or joining two lines into a block, changes which `%var%` value is read. Reflow inside one block is safe.
9. Delimiters: space, tab, `,`, `;`, `=` all separate arguments (`copy a,b`, `cd;..`, `echo,`), so replacing one by another changes nothing for most commands and everything for others (`for /f "tokens=1,2"` options are inside quotes, which are never touched). Do not normalize them.
10. Quotes: adding, removing or moving quotes changes meaning (`set "x=1"` vs `set x=1`, `if "%a%"==...` vs unquoted with an empty variable). Quote style is never a formatter option.
11. Case: command, keyword, label and `%var%` names are case-insensitive, so keyword case is safe; `for` loop variables `%%i` and `%%I` are different variables; `if "A"=="a"` is case-sensitive without `/i`; file names are not ours to change. Do not touch case inside arguments, strings, variable names or labels (labels are case-insensitive but goto targets in other files / `call :Name` are display names).
12. `%%` vs `%`: batch file context vs command line; caret and `!` escaping differ when delayed expansion is enabled (`^^` needed). The formatter must not rewrite escapes.
13. Line endings and encoding: cmd.exe has a known label-search bug on LF-only files (`goto`/`call :label` can fail at 512-byte boundaries), so LF to CRLF is a behaviour change, in the good direction; a UTF-8 BOM at the start breaks the first command (`'ï»¿@echo'`): removing it is a fix, adding it never. 25 corpus files (1.8%) are not UTF-8: the formatter must refuse or keep the bytes (the lossless tree is UTF-8 today).
14. Blank lines and indentation of ordinary commands, the case of keywords, a final newline, and trailing blanks outside points 1 and 2 are safe. Blank lines inside a caret continuation are not (they end it).

## 4. Lint leg

### Tools (evidence)
| Tool | Evidence |
|---|---|
| **Blinter** (`BatchLang/Blinter`, Python 3.11+, AGPL-3.0, also standalone binaries x64/arm64) | 193 rules (E 46, W 69, P 26, S 28, SEC 24), `blinter.ini` config, `REM LINT:IGNORE` suppression, fix suggestions; PyPI `Blinter` 1.1.74, **20,445 downloads in the last month**; 54 stars; pushed 2026-11-16; VS Code extension `14ag.blinter` (6,333 installs on the Marketplace, 17,956 on Open VSX) |
| batIDE (`gdxml/batIDE`) | C#, MIT, 0 stars, 123 rules, pushed 2026-11-22: too new to count |
| `crosenblum/blint`, `sed-23/scriptkit`, `windows-encoding-preflight-lint` | 0-3 stars, tiny |
| shellcheck / PSScriptAnalyzer / mega-linter / pre-commit | no cmd support |

Blinter is therefore the primary tool of the leg (one maintained tool, usage evidence): "none found" in `languages.yaml` is wrong, and the gain of the lint leg is 0.7 (Python runtime), not 1.0. Its rules are the checklist source (facts: rule names and what they report, not code: AGPL; using it as a reference tool for parity runs is fine, copying code is not). **I did not run Blinter**: running downloaded code was out of bounds for this task. A parity step must run it in a venv on the corpus first (yield per rule, precision) before the leg's ranking is trusted: Blinter's own precision on real code is unknown to me (193 rules, about 45 are security and performance heuristics that tend to be noisy).

### Checklist a no-reference leg would have (and what Blinter adds)
Sources per no-reference-tool.md: the cmd.exe documentation (`cmd /?`, `for /?`, `if /?`, `set /?`, `call /?`, `goto /?`), the DosTips and SS64 pitfalls, Blinter's rule list (193), the fix commits of the corpus projects (not searched). Entries that need running code or the machine (E005 path length, E027 UNC cwd, W006 network timeouts, SEC005 privilege, W043 process checks) are named and left out.

### Mapping to what exists (names judged from Blinter's `spec/data/rules.yaml`, not by reading each checker; counts are approximate)
| Group | Blinter rules (examples) | About | Covered by |
|---|---|---|---|
| Syntax shapes on the tree | E001 parens mismatch, E003/E004/E036 if forms, E009 quotes, E010 for without do, E014/E015 colon in call/goto, E039/E040/E042/E043/E044 block layout, E046 redirection, W021 unquoted if, W044 `set x = v`, W055/W056 if defined and pseudo AND/OR | 60-70 | pattern rules (YAML) over grammar kinds, once the parser has the node kinds; the parse error covers E001 |
| Names and scopes | E002 missing label, W013 duplicate label, E006/P008/W022 undefined variable and delayed expansion, W068/W071 same-block percent expansion, P005/P006/P024/P026 setlocal pairing | 12 | `scope-rules` (unused, undefined, redeclared from a scope table: labels and `set` names as declarations, `goto`/`%x%` as reads), `no-duplicate-sections` for labels; setlocal/endlocal pairing has no shared check (a paired-commands check would also serve `pushd`/`popd` W061 and shell `trap`: a second user is plausible) |
| Flow | E008 unreachable after exit/goto, W004 infinite loop, W001 missing exit code | 5 | `unreachable-code` (flow family, flow graph built from the tree: goto and exit as jumps) |
| Lines and bytes | S004 trailing whitespace, S005/E018 line endings, E048/S011/S020 line length, S003 command case, S012 indent, S027 blank lines, E032 caret with trailing blank, E035 smart quotes, W012/W030 non-ASCII, W032 charset | 20 | `max-family` (line length), `hard-tabs`, `token-case-lint`, `statement-indent` and `statement-blank-lines` (cmake-lint; single-use today: Batchfile would be their second user, which is good for the second-use rule); **missing shared**: line-ending, BOM, smart-quote and non-ASCII checks (generic over any language, none exists) |
| Command tables | E034 removed commands, W024 deprecated, E045 invalid switch, E012/E013 call/command forms, W009/W010/W029 compatibility, W017/W028 errorlevel semantics | 10 | `documented-command`/`missing-arguments` are the nearest; a table of commands, switches and deprecations needs a shared reader (facts as rows: internal command, valid switches, status) |
| Security and performance heuristics | SEC001-SEC025 (injection, unsafe temp, hardcoded paths, credentials, registry, fork bomb, autorun), P001-P027 (redundant checks, inefficient loops, duplicated code) | 45 | pattern rules for the command-shaped ones; `secrets` exists as an id in other languages; most of P and a third of SEC are opinion rules whose judged precision is the open risk |
| Rest | documentation/style opinions (S008, S009, S013, S014, S018, S019, S023, S024) | 8 | `documented-command`, `max-family`; low value |

Existing shared rules cover roughly 55% of the 193 by mechanism (tree patterns, scopes, flow, max/case/indent/tabs), a further 10% needs 3 small shared mechanisms (line endings/BOM/non-ASCII bytes, command tables, paired commands), and about 25% are heuristics whose value is unproven. This agrees with `parts: 2-3` in the cost; I set it to 2 for the corrected score. The 85% bar against Blinter on the same lines is reachable only for the first two groups; the SEC/P half decides the number, so the parity step should rank Blinter's rules by yield on this corpus before building anything.

## 5. Corrected cost and score

Formula (`xtask/full/src/newlang.rs`): `4 x 85 x Demand x sum(Gain) / (20 x (Parser + sum Cost))`, Demand `0.512~` (815,225 pushers measured; the Stack Overflow survey does not list Batch, so half of it stays an estimate: 0.31 from pushers, 0.71 from the 2.27 factor; unchanged).

| Part | languages.yaml | Measured | Why |
|---|---|---|---|
| Parser | ~1.2 (tree-sitter) | **1.2** tree-sitter wharflab (but 28.6% of files refused) or **1.5** hand-written | section 2; parser 1.2 does not price the refused files, 1.5 is the honest price of a usable result |
| Lint gain | 1.0 (none found) | **0.7** (Blinter, Python runtime) | section 4 |
| Lint cost | 1.3 (parts 3) | **1.2** (base 1 + 2 parts: bytes checks, command tables; paired commands as the third is optional) | section 4 mapping |
| Format gain | 1.0 | 1.0 (no maintained tool; the one partial tool is poor) | section 3 |
| Format cost | 1.3 (width +0.2, parts 1) | **0.8** (whitespace -0.3, parts 1) | wrapping is unsafe and the corpus decides almost nothing: whitespace only |

| Plan | As in the queue | Tree-sitter (1.2) | Hand-written (1.5) |
|---|---|---|---|
| lint | 3.48~ | 2.54 (Blinter) | 2.25 |
| format | 3.48~ | 4.35 | 3.78 |
| both | 4.58~ | **4.62** | **4.22** |

Rows to compare (queue values, all still estimates `~`, except Ruby and Dockerfile/Makefile demand): Angular templates lint 3.72~ (parser paid, angular-eslint, Node), Gradle lint 3.51~ (both 3.20~), Jupyter both 2.59~, Ruby format 2.47~ (parser paid), Kubernetes lint 2.33~. The retrofit rows Dockerfile 10.55~ and Makefile 6.75~ rank on their own rule (never ahead of a primary leg).

Score of Batchfile after measuring: 4.22 to 4.62 for "both", still above every other new-language row of the queue (Angular 3.72~). The ranking holds on the formula. The numbers behind it are weaker than the ranking suggests:
- Almost all of the format leg's score comes from gain 1.0 for "no tool" and a cheap `whitespace` layout. The measurement says the leg has little to decide (only keyword case passes the 80% corpus rule, indentation is 4 sp 38-43% and stays as written) and a lot it must not touch (14 places above). Reaching `agreement` 80% is nearly automatic; what users get is small (indent by block depth, blank-line runs, case). The formula cannot see value, only effort, and treats a leg that mostly says "as written" like a full formatter.
- The lint leg is where users would see value (Blinter has 20k downloads/month, so people want checks), but its reference exists now, which lowers its gain (0.7) and fixes the target: 85% of Blinter's findings on the same lines, with about 45 heuristic rules inside the target. A first lint step needs a Blinter run on this corpus (not done) before its true cost is known.
- Parser risk is the largest unpriced term: tree-sitter refuses 28.6% of real files; hand-written takes more code. Hand-written and the Blinter-gated lint is 2.25 on its own, which puts lint-only below Angular lint (3.72~) and Gradle lint (3.51~) on their estimates.
- Angular lint (3.72~) has a parser Sudsy already owns, a maintained reference (angular-eslint), and the HTML machinery, so its estimate carries less uncertainty than a new parser and a no-reference format leg; its number is also unmeasured, so this comparison is estimate against estimate.

### Opinion
Batchfile is a legitimate top candidate by the formula but should not be built as "both" next. If it goes next: (1) lint first with Blinter as the reference, hand-written lexer, the first step measuring Blinter on this corpus (its yield per rule decides which of the 193 are worth a rule); (2) the format leg as a small whitespace formatter after, not a width formatter; (3) update `languages.yaml`: lint tool Blinter (gain 0.7), format `layout: whitespace`, parser 1.5 hand-written. If the lead wants the best expected value per slot, Angular templates lint or Gradle lint should be measured first: both have a maintained reference, no new parser risk and cleaner parity bars. Batchfile's honest estimate is a medium-size language (Windows scripts, 815k pushers) with the hardest parser of the top rows and the thinnest formatter. Your decision: parser choice (hand-written recommended), and whether the format leg is wanted at all.

## Files
- Corpus script and pins: `.cargo/tmp/batch-fetch.sh`, `.cargo/tmp/pins.txt`; file list and split `.cargo/tmp/batch-files.txt`; corpus stats `.cargo/tmp/stats.out`; probe snippets `.cargo/tmp/batch-snips.txt`; harness `.cargo/tmp/meas-main.rs`, `meas-hand.rs` (build with a `build.rs` that compiles the three `parser.c` files with `-D tree_sitter_batch=ts_w` style renames); slot outputs in `/scratch/slots/sudsy/8/meas` (`err-*.tsv`, `errs-wharflab-nl.txt`), downloads in `/scratch/slots/sudsy/8/dl` (grammars, Blinter README and `rules.yaml`, never executed).
