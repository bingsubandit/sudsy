# Kotlin (`lang-kotlin`)

## Parser
[ktrs-parser](https://github.com/Hexay/ktrs) 0.5.1 (MIT OR Apache-2.0) with `ktrs-syntax`, pinned to the exact version in the root `Cargo.toml` (dependabot ignores them; a bump is a deliberate change of `grammars/kotlin.grammar` and `crates/lang-kotlin/src/adapter.rs`, followed by the conformance run). It is a port of the Kotlin compiler's PSI parser, so node and token names are the compiler's (`FUN`, `VALUE_PARAMETER_LIST`, `LONG_STRING_TEMPLATE_ENTRY`), the ones ktlint's rules are written over. `.kt` files are source files, `.kts` (Gradle Kotlin DSL, scripts) are scripts; the file id decides.

`adapter.rs` walks ktrs's flat tree once and hands tokens and nodes to the shared `replay`:
- a node is the row of `grammars/kotlin.grammar` named like its PSI type in Pascal case (`FUN` is `Fun`); a PSI type without a row is transparent;
- a token is ours by its kind: every keyword (hard, soft, modifier) is `Keyword`, strings are `StringTemplate` nodes of `OpenQuote`, `StringPart`, `EscapeSequence`, `$` / `${` entries and `ClosingQuote`, and other punctuation is the grammar token with that text;
- whitespace, comments and KDoc are `replay`'s gaps (`//`, nested `/* */`, a `#!` first line) and follow the shared attachment rule; KDoc is a `Comment` like the others, its structure is not in the tree;
- an `ERROR_ELEMENT` is an `ErrorElement` node and a parse error with ktrs's message; tokens the parser left out of its tree become `Unknown` tokens with an error.

What ktrs needs from its caller, done in the adapter:
- it requires `\n` line breaks: the text is normalized and every offset mapped back (`crs`, a binary search), so a CRLF file keeps its line breaks;
- it recurses once per nesting level and aborts the process on overflow, so the parse and the walk run on a thread with a 256 MB stack, and a scan refuses text nested deeper than 2,000 brackets or 2,000 prefix operators; it is exponential in nested annotation arguments (`@A(@A(...))`, 4 s at 22 levels), so more than 12 are refused; a panic is caught; a file over 16 MB is not parsed. All of these leave the file as one `Unknown` token and one error, which the formatter and the linter refuse.

Failure modes of the parser itself: it follows the compiler of Kotlin 2.4.20 and rejects the newest experimental syntax of the compiler's master (union types, `error` as a modifier); it is quadratic on a chain of tens of thousands of binary operators.

## Options
<!-- generated:options -->
| Option | Values | Default | Description |
|---|---|---|---|
| `indent_size` | int 1-16 | `4` | Columns per indentation level (also the width of a tab). |
| `line_width` | int 20-1000 | `140` | Preferred maximum line width in columns. |
| `indent_style` | `space`, `tab` | `space` | Indent with spaces or tabs. |
| `final_newline` | bool | `true` | End the file with a newline. |
| `end_of_line` | `auto`, `lf`, `crlf` | `auto` | Line break style of the output: `lf`, `crlf`, or `auto` to keep the file's first line break. |
<!-- /generated -->

## Formatting
Layout is our own code over the lossless tree (`crates/lang-kotlin/src/format/`): tokens are never changed, added or removed (so no trailing commas, import sorting or wrapping), and the layout follows ktlint's `ktlint_official` style where it is a matter of whitespace:
- spacing between tokens (`space.rs`): after commas, around binary operators, `->`, `=`, colons by role, braces, keywords before `(`, none inside brackets or around `.`, `?.`, `::`, `!!`, ranges; tokens without a rule keep a blank as written, collapsed to one;
- line breaks are kept as written, blank lines collapse to one, and `blank.rs` sets the blank lines ktlint wants: before declarations of a file, class body or block (not between consecutive properties or type aliases), after the package, the imports and the file annotations, none after an annotation or after the `{` of a body or block, none before `}`;
- indentation (`indent.rs`) is relative to the anchor line of the enclosing construct (class header, call chain, argument or parameter list, `when`, lambda), one unit per level; continuation lines of a header, expression or chain hang one unit; text inside strings is never touched; KDoc and block comments are aligned under their opening `/*`;
- the output ends with one line break.

Not done (ktlint does them, they change tokens or wrap lines): the wrapping rules (function signatures, argument and parameter lists, class signatures, chains, long expressions), trailing commas, import ordering and wildcard handling, `string-template-indent`, and the rules that rewrite code. They are gaps against `scripts/parity.sh format ktlint`.

## Rules
Pattern files (`crates/lang-kotlin/rules/ktlint.yaml`) for the ktlint rules that are about names and structure rather than layout: `no-wildcard-imports` (on), `no-unit-return` (on), `no-empty-class-body`, `package-name`, `class-naming`, `enum-entry-name-case`, `no-empty-file` (off). The shared `max-line-length` and `comment-space` cover two more. The catalog `docs/parity/ktlint.yaml` lists every ktlint rule with its status; the layout rules are the formatter's.

## Parity
`scripts/parity.sh format ktlint` (ktlint 1.8.0, `ktlint_official`, see [../development/parity.md](../development/parity.md)) over the 5,776 corpus files it can parse: the ratchet counts the files that match byte for byte (about a quarter: most of the corpus is not ktlint-formatted, and ktlint wraps and rewrites more than the layout above); a second format changes no file and the safety check refuses none. Against ktlint restricted to its whitespace, blank-line and indentation rules the match is 95%.

## Conformance
`bash tests/conformance/fetch-kotlin-corpus.sh`, then `cargo test --profile corpus -p lang-kotlin --test integration -- --ignored --nocapture conformance`: kotlinx.coroutines, ktor, nowinandroid, ktlint and the Kotlin standard library, `.kt` and `.kts` (6,632 files, 20 s in the `corpus` profile): every file parses without errors, prints back byte for byte and satisfies the trivia rule; the 322 compiler PSI parser test inputs (`testData/psi`) that hold deliberate errors are reported, not required to parse. An unclosed block comment is an error.
