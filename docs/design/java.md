# Design: Java

Goal: the formatter and linter Java users switch to: google-java-format layout (Google style, AOSP as options) and Checkstyle's `google_checks.xml` / `sun_checks.xml` as the rule reference. Built in the steps of section 3; this note is deleted when the last step lands.

## 1. Parser comparison (measured, not committed)
Linux arm64 Graviton, rustc 1.99.0, release with LTO, parse of the whole file from memory, warm, no adapter. Corpus: `tests/conformance/fetch-java-corpus.sh` (Guava, Gson, Error Prone, Commons Lang, google-java-format incl. its test inputs): 5,337 files, 48.8 MB. Binary: a stripped release binary containing only the parser and a `main` (tree-sitter-java includes the tree-sitter runtime, which Sudsy links already).

No Rust-native Java parser with a lossless tree exists besides the ones below (crates.io search: "java parser/ast/syntax/lexer"; ANTLR runtimes need a generated parser from a grammar and the forked ANTLR tool, not tried).

| | tree-sitter-java 0.23.5 | rezel-lang-java 0.0.0 | feijoada 0.1.0 | cure-java-parser 0.4.4 |
|---|---|---|---|---|
| Kind | C grammar, official | Rust, LR (Lezer grammar port), concrete tree incl. punctuation and comments | Rust, hand-written, typed AST holding every token position and trivia | Rust, hand-written, semantic AST (signatures, bodies), no token-level tree |
| Parse speed, whole corpus | 12.5 MB/s | 16.8 MB/s recovering, 22.7 strict | 159 MB/s | 74 MB/s |
| Corpus files failing / with errors | 14 (13 are Guava/Commons files using `Object @Nullable ... args`, i.e. a type annotation on varargs; one Commons file with an unicode-escape identifier) | 2 (a text block containing `\"""`, a lexer bug; strict mode fails the same) | 0 | 0 (but see recovery) |
| Mutated files (one `)`, `}` or `;` deleted, 271 of 309 became invalid) | 271 reported, 0.57% of the input inside error nodes | 271 reported, zero-width `⚠` recovery nodes, 0.07% inside | 271 rejected, whole file lost (no recovery) | 128 reported: accepts about half the broken files without a diagnostic |
| Java version probes (18) | fails: `import module` and flexible constructor bodies (Java 25), unicode escapes in identifiers; accepts the removed string templates | fails only the removed string templates (Java 26 claimed, checked against javac) | same as rezel | same as rezel |
| Stripped binary (standalone) | 803 KB | 595 KB | not built | not built |
| New crates / deps | none (runtime shared) | `rezel-common`, `rezel-lr`, `unicode-ident`, `zerocopy`, plus a 68 KB table | none | `cure-java-ast`, `cure-engine`, `cure-tree` |
| Upstream | tree-sitter org; last release 0.23.5 Dec 2024, only CI commits since; 14 M downloads | one author (magic-akari), 1 version, created Sep 2026, 0 stars, MIT/Apache-2.0; active (push Nov 2026) | one release Oct 2026, GitLab | 20 versions since Oct 2026, one author, MIT |
| Fit with our tree | grammar rows in `lang.yaml` mapping, shared `core-treesitter`; no new adapter code; whitespace rebuilt by the shared attach | adapter replay (`provider_api::adapter`): walk `SyntaxNode`, one kind row per node name (about 160 names, the way `lang-python` does it); tokens are nodes, whitespace is a gap | typed structs per node (about 200 types, no generic walk): an adapter would be one function per type; `Visit` gives tokens and trivia but no node boundaries | an AST, not a tree of tokens: cannot rebuild a lossless tree |

Reading: feijoada and cure are out (no recovery, or no lossless tree; cure also accepts invalid files silently). The choice is tree-sitter-java or rezel.

- tree-sitter-java: no new code and no new dependency, but it stops at about Java 21, rejects the JSpecify varargs form Guava uses (a grammar fork would carry that, see `docs/development/grammar-forks.md`) and has been unmaintained for a year. Formatter and linter must refuse files with errors, so these are refusals on real code.
- rezel-lang-java: the broadest and most current syntax (Java 25/26, javac-checked), faster and smaller, error recovery as good. Risk: a 0.0.0 crate by one maintainer, two months old; pinned exactly like oxc, ruff and sqruff (a bump touches the kind table and adapter only), and a hand-written parser can replace it later without touching anything outside the crate. Cost: an adapter (about as much as Python's or SQL's) and the escaped text block bug (2 files; report upstream or patch).

**Decision (2026-11-16, the user): rezel-lang-java through adapter replay**, pinned exactly like oxc, ruff and sqruff (`=0.0.0`, Dependabot-ignored; a bump touches the kind table and adapter only). Reasons: the broadest and newest syntax (Java 25/26, checked against javac) where tree-sitter-java stops near 21 and rejects the type-annotated varargs Guava uses; faster (16.8 against 12.5 MB/s) and smaller (595 against 803 KB standalone); recovery as good (`⚠` points); a hand-written parser can still replace it without touching anything outside the crate. tree-sitter-java is dropped. Known upstream defects: a text block containing an escaped `\"""` fails to lex (2 corpus files), and the grammar leaves `<`, `>`, `|`, `&` and `@interface` out of the tree (the adapter reads them from the text between leaves).

## 2. Targets
- Formatter: safe (second format identical, no file refused, meaning preserved) and about 85-90% of real-world files identical to google-java-format; the tool's own tests only as far as real-world work brings them.
- Lint: the common bug-catching Checkstyle checks and what the google preset needs; the rest planned in `docs/parity/checkstyle.yaml`.

## 3. Steps
| Step | Builds |
|---|---|
| 1 | `lang-java`: parser, lossless tree, provider and registration, `.java`, verbatim formatter, corpus script, bench inputs, docs |
| 2 | Formatter: google-java-format layout, `scripts/parity.sh format google-java-format`, options for AOSP (4 spaces) |
| 3 | Lint: Checkstyle catalog `docs/parity/checkstyle.yaml`, rule families, `java/google` preset (options and rule levels only) |

Then "Java: generalization step" under Small tasks in `docs/roadmap.md`.
