# Design: Rust evidence

Measurements before any Rust product code. Reference tools exist (rustfmt for format, clippy for lint), so this page is the evidence for the choices a formatter or a linter makes on top of them, and for the parser (the user decides; no parser is built). Scope: `*.rs` files of the Rust 2015, 2018, 2021 and 2024 editions. Toolchain of the measurements: rustfmt and clippy of the repository pin (`rust-toolchain.toml`, 1.99.0). Machine: AWS Neoverse-V2 aarch64, 8 cores. Looked up 2026-11-26. Scripts: `scripts/evidence_rust*.py|sh`, `scripts/evidence_rust_parsers/` (the parser harness), `scripts/evidence_rust_probes.sh` (rustfmt verdict on the probes).

## Corpus
952 `.rs` files in 594 projects (11.1 MB), pinned in `tests/conformance/fetch-rust-corpus.sh`. The corpus folder holds 4,270 `.rs` files on disk; 692 under test-data folders (`ui`, `source`, `target`, `fixtures`, `test_data`, `testdata`, `test-data`: deliberately unformatted or non-compiling code) are left out, and a project keeps at most 30 files (the first by path). The cap was fixed before the counts were read.
- Batch 1, 12 public projects pinned by commit (default-branch heads of 2026-11-26): ripgrep, bat, fd, tokio, serde, anyhow, clap, hyper, alacritty, starship, mdBook, rust-analyzer. 355 files (the cap reached by 11 of them).
- Batch 2, the first 1,000 items of the GitHub code search `extension:rs` (ten pages, `tests/conformance/rust-search.tsv`, pinned by blob sha). The search repeats items across pages: 612 distinct files in 596 repositories; 597 of them remain after the cut. A single file has no `Cargo.toml`, so its edition is not known; rustfmt runs as edition 2021 for it (a stated default).
- Split by project, seed 20261204: 297 deciding, 297 held out (`python3 -I scripts/evidence.py rust list`).
- Files that are not UTF-8 are left out of every count (one file).

## Tools (usage evidence)
Usage (`scripts/evidence_rust_usage.sh`, code search totals, lookups 2026-11-26; the totals are the API's, not a sample):

| Tool or key | Evidence |
|---|---|
| rustfmt (official, ships with rustup) | `filename:rustfmt.toml` 47,360 hits, `filename:.rustfmt.toml` 8,624; `.github/workflows` with `cargo fmt` 170,240; `.pre-commit-config.yaml` with rustfmt 1,864 |
| rustfmt keys | `max_width` 25,152 files, `imports_granularity` 16,704, `group_imports` 13,408, `newline_style` 13,408, `use_small_heuristics` 12,224, `style_edition` 3,560 |
| rustfmt configs in the corpus | 7 files: ripgrep (`max_width = 79`, `use_small_heuristics = "max"`, `edition = "2024"`), alacritty (many options: `imports_granularity`, `match_block_trailing_comma`, `overflow_delimited_expr`, ...), mdBook (`style_edition = "2024"`), rust-analyzer (`reorder_modules`, `use_small_heuristics`), bat, fd and starship (defaults, comment only) |
| clippy | `filename:Cargo.toml "[lints.clippy]"` 53,824 files, `[workspace.lints.clippy]` 31,232, `filename:clippy.toml` 26,880; `.github/workflows` with `cargo clippy` 159,744; `.pre-commit-config.yaml` with clippy 5,416 |
| clippy lints named in code | `clippy::pedantic` 57,600 hits, `clippy::nursery` 20,288, `clippy::unwrap_used` 396,800 (mostly `allow` in prose and in code) |
| clippy in the corpus | 78 of 113 `Cargo.toml` files have a `[lints...]` table (rust or clippy); `clippy.toml` in 3 files, with no key line |

Editions of the `Cargo.toml` files above the corpus files: 2021 in 8 of 25 manifests, 2024 in 3, no `edition` key in the other 14 (the rest of the corpus files are single files, see Corpus).

## Format
rustfmt is the reference. The corpus is read without running rustfmt for the decision table (`python3 -I scripts/evidence.py rust table`; one vote per deciding project, agreement is the share of the held-out occurrences with the default). The rows are the default choices of rustfmt:

| Row | Votes (deciding projects) | rustfmt default | Held-out agreement |
|---|---|---|---|
| indent of a line | 275 spaces, 6 tab (281) | spaces | 99.6% (130,878 of 131,369) |
| indent width of an indented line | 274 multiple of 4, 2 other (276) | multiple of 4 | 98.6% (129,065 of 130,872) |
| trailing blanks of a line | 297 none (297) | none | 100.0% (163,227 of 163,280) |
| final newline | 278 yes, 19 no (297) | yes | 96.9% (492 of 508 files) |
| line endings | 294 LF, 3 CRLF (297) | LF (rustfmt's `newline_style` is Auto: a CRLF file stays CRLF, not decided here) | 99.6% (506 of 508) |
| blank lines at the start of a file | 293 none, 4 present (297) | none | 99.2% (504 of 508) |
| blank lines at the end of a file | 294 none, 3 present (297) | none | 98.8% (502 of 508) |
| blank lines in a row | 275 one, 1 three or more (276) | one | 99.8% (15,866 of 15,891) |
| blank line after a line ending with `{` | 280 none (280) | none | 100.0% (23,087 of 23,089) |
| line width | 296 within 100, 1 over 100 (297) | within 100 (`max_width` 100) | 99.1% (161,808 of 163,280) |
| brace of an item head with one line | 264 same line, 4 other, 1 own line (269) | same line | 87.0% (9,557 of 10,989) |

Pooled held-out agreement over the rows: 635,492 of 640,802 = 99.2% (mean of the rows' project means 98.2%). The agreement is high because most of the corpus is rustfmt-formatted (60.7% of the files are unchanged by it, below), so it says little about the choices the rows do not cover. The brace row is the one under 90%: a head of several lines (the `other` votes) and an own-line brace are not the default. A row for the comma after the last item of a `(` or `[` list was tried and dropped: a line rule cannot tell a call's last argument from a struct or array item, so the vote mixed two rustfmt rules.

rustfmt on the corpus (`python3 -I scripts/evidence_rust_tools.py <work-dir>`; stdin, the file's edition from its `Cargo.toml`, 2021 when none, the repository's `rustfmt.toml` when it has one; the outcome per file):
- unchanged 578 of 952 (60.7%), changed 316 (33.2%), parse error 58 (6.1%).
- by edition: 2021 501 unchanged, 316 changed, 58 parse errors; 2024 77 unchanged, no error.
- of the 58 parse errors, 27 are `let chains are only allowed in Rust 2024 or later` in an edition 2021 file; rerun as 2024, 27 parse (25 unchanged, 2 changed) and 31 fail in both editions (the messages: `box_syntax` removed, `expected item after attributes`, raw pointer types, `try` macro, unclosed delimiter, ...). The sets of the 27 and of the messages were not compared file by file.

Not done: stability (the formatter does not exist yet), harm sample, meaning oracle (rustc's own check of a formatted file is the natural one), and the choices rustfmt makes that the table does not cover: wrapping of call arguments and chains, import grouping and order, match arms, `where` clauses, comments and doc attributes. Those are the rows a formatter must decide by rustfmt's rule set, not by a corpus vote: a formatter that follows the default `rustfmt` rules is the reference, and the corpus shows what the projects keep (60.7% of the files unchanged).

## Lint
clippy is the reference. Two numbers from the corpus (`python3 -I scripts/evidence_rust_lints.py`, lint levels and attributes as written; groups counted once):
- In `.rs` files, clippy lints set by `#[allow]`, `#[warn]`, `#[expect]`, `#[deny]` (occurrences): allow 250 (109 lint names), expect 8, warn 6, deny 2. Most often allowed (occurrences, projects): `derive_partial_eq_without_eq` (14, 2), `too_many_arguments` (7, 7), `unwrap_used` (6, 6), `wildcard_imports` (6, 3), `expect_used` (5, 5), `too_many_lines` (5, 4).
- In `[lints.clippy]` and `[workspace.lints.clippy]` tables: allow 100 (98 lint names), warn 75 (70 lint names), deny 3. The group rows (`pedantic`, `restriction`, `correctness`, `perf`, `complexity`, `style`) are set in 1 or 2 projects each; one table sets `all = "allow"` with `correctness`, `complexity` and `perf` on, so the projects that set a level keep clippy's default groups with their own changes.

The lint leg's checklist (findings of clippy's default set on the corpus, with the bad and good example per lint) is not run: a clippy run needs each project's dependencies and types (`cargo clippy` in its workspace), and most of the corpus is single files. The default set of clippy (`clippy::all`: correctness, suspicious, style, complexity, perf) is the candidate for the preset; the choice among the groups is the user's.

## Parser candidates
Measured with `scripts/evidence_rust_parsers` (one binary per candidate, each linking its own parser; release, LTO, one codegen unit, strip; best of 5 whole-corpus parses from memory, warm; the file list and editions of `scripts/evidence_rust_tools.py`). Commands: `cargo build --release --manifest-path scripts/evidence_rust_parsers/Cargo.toml --features ts --bin ts` (and `syn`, `ra`; `--bin base` for the empty program).

| Row | tree-sitter-rust 0.24.2 (on tree-sitter 0.27, ABI 15) | syn 3.0.10 (`parse_file`) | ra_ap_syntax =0.0.364 |
|---|---|---|---|
| files with a parse error (of 952) | 34 (3.6%) | 30 (3.2%) | 33 (3.5%) |
| parse error set against rustfmt's 58 | 27 in both, 31 only rustfmt, 7 only tree-sitter | 30 in both, 28 only rustfmt, none only syn | 31 in both, 27 only rustfmt (let chains in 2021 files, see Format), 2 only ra |
| parse speed, whole corpus (11.1 MB) | 1.20 s, 9.3 MB/s | 0.58 s, 19.1 MB/s | 0.47 s, 23.7 MB/s |
| stripped binary (empty program 267 KB) | 1,578 KB (+1,311 KB) | 988 KB (+721 KB) | 791 KB (+524 KB) |
| crates in the build (`cargo tree`, normal deps, unique) | 10 | 4 | 33 |
| clean release build (wall time, 8 cores) | 13 s | 13 s | 19 s |
| damaged files (one middle closing bracket removed, 938 files): reported | 91.9% | 92.0% | 92.1% |
| damaged files: share of the file in error nodes, median / p75 | 0.0% / 0.2% | none (no tree) | 0.0% / 0.0% |
| probes parsed (18, `scripts/evidence_rust_parsers/probes`) | 16 | 14 | 18 |
| trivia (comments, whitespace) as tokens of the tree | tree-sitter has comment nodes and no whitespace tokens; not measured beyond that | the AST drops comments and whitespace (crate design); not measured | lossless rowan tree: every token, trivia included (crate design); not measured |
| licence, last release, last push (GitHub) | MIT; 0.24.2 on 2026-03-27 (grammar crate); repository pushed 2026-09-16; 538 stars | MIT OR Apache-2.0; 3.0.10 on 2026-11-09; pushed 2026-11-23; 3,469 stars | MIT OR Apache-2.0; 0.0.364 on 2026-11-23 (weekly `0.0.N` releases); pushed 2026-11-24; 17,018 stars (rust-analyzer) |
| downloads (crates.io, total / last 90 days) | 26.7 M / 8.9 M | 3.2 B / 968 M | 4.2 M / 1.1 M |
| maintainers | tree-sitter organisation (the grammar repository); funding not read | one author (dtolnay), widely used as the proc-macro parser | the rust-analyzer team of the rust-lang project; the crate is a publish of its parser, with no stable API (exact pin) |
| minimum Rust | not declared | 1.71 | 1.98 (declared) |

Probes (the construct, the edition it is written for, rustfmt's verdict in `scripts/evidence_rust_probes.sh`, and each parser's):

| Probe (edition) | rustfmt | tree-sitter | syn | ra |
|---|---|---|---|---|
| let chain (2024) | ok | ok | ok | ok |
| let chain (2021) | error | ok | ok | ok |
| `gen` block (2024) | ok | ok | error | ok |
| `unsafe extern` block (2024) | ok | error | ok | ok |
| `#[unsafe(no_mangle)]` (2024) | ok | ok | ok | ok |
| precise capturing `use<'a>` (2024) | ok | ok | ok | ok |
| async closure, raw borrow `&raw`, let-else, never type, const generics, GAT, raw identifier, inline const (2021) | ok | ok | ok | ok |
| `async` identifier (2015) | ok | ok | error | ok |
| `dyn` identifier (2015) | ok | ok | error | ok |
| `try!` macro (2015) | ok | error | ok | ok |
| bare trait object `&Fn()` (2015) | ok | ok | error | ok |

Reading: the parsers accept the newest syntax (tree-sitter misses `unsafe extern` and `try!`; syn misses `gen` blocks and the 2015 forms: keyword identifiers and bare trait objects). None of the three gates an edition-only construct (let chains in 2021 are accepted by all three); rustfmt does. An edition check is therefore a rule of ours or a rustfmt-like step, not a parser error. The 2 files only ra rejects (and the 7 only tree-sitter rejects) are not inspected: the messages were not printed for them.

Edition input: tree-sitter ignores the edition; syn has none; ra takes it, and a file's edition comes from its `Cargo.toml` (`edition` key; 2021 for a single file).

Damaged files: a removed bracket is detected in about 92% of the cases by every parser; tree-sitter and ra both keep the rest of the file in a tree (the median share in error nodes is 0%); syn gives no tree of a damaged file, so the file is one error.

Reading for the choice (the user decides):
- ra_ap_syntax: the only candidate with a lossless tree (trivia in the tree, as an adapter needs), error recovery, the most probes parsed (18 of 18), the fastest (23.7 MB/s) and the smallest binary. Costs: 33 crates; the crate is a publish of rust-analyzer's parser with no stable API, so it is pinned exactly and moved each release (the same as the oxc and ruff adapters, see docs/ai/architecture/adding-a-language.md); the adapter maps ra's kinds to our grammar rows (`lang-js` is the worked example of that work).
- tree-sitter-rust: the cheapest integration in this repository (a grammar file and a `lang.yaml`, the `core-treesitter` engine; `docs/ai/architecture/adding-a-language.md`); the grammar misses `unsafe extern` blocks (2024) and the `try!` macro (2015) of the probes, it is about 2.5 times slower than ra, and its corpus error count is within one file of ra's.
- syn: rejected. No tree on an error, a lexer that drops comments and whitespace, and the 2015 keyword forms are errors.

Recommendation: ra_ap_syntax =0.0.364 behind an adapter (the lossless tree, recovery, newest syntax, speed). The alternative is tree-sitter-rust 0.24 when the adapter's cost is the deciding fact. Edition rules (let chains and the other 2024 forms) are ours, read from the file's edition. Nothing is built; the choice and the pin are the user's.

## Not verified
- Parser: comment and whitespace tokens (the trivia row) were not measured beyond the crate designs; the adapter's cost (kinds, trivia attachment) is an estimate; deep nesting (stack depth at which each parser fails) was not measured; the 2 files ra rejects and the 7 tree-sitter rejects were not read.
- Build times are one clean build each, not best of several.
- Format: the stability, harm and meaning checks (no formatter exists); the choices beyond the decision table (call arguments, chains, imports, match arms, comments) are not measured; the corpus is 60.7% rustfmt-clean, so a formatter's agreement with rustfmt is not estimated from it.
- Lint: the clippy checklist and the precision sample were not run (needs cargo and project types).
- Batch 2 files are single files: their edition is assumed 2021 (rustfmt and the parsers), and their configuration is their project's, which is not known.
- Usage totals of code search are approximate API counts; the star and download counts are as of 2026-11-26.
