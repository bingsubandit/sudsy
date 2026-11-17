# Java (`lang-java`)

## Parser
A hand-written lexer and recursive-descent parser (`lexer.rs`, `parser/`: `decl.rs` files and declarations, `stmt.rs` statements, `switch` and patterns, `expr.rs` expressions, `types.rs` names, annotations and types, `look.rs` lookahead) reads Java up to 26 (records, sealed types, switch patterns and guards, unnamed variables, text blocks, `import module`, flexible constructor bodies) and builds the lossless tree of `grammars/java.grammar` directly (`TreeBuilder` with checkpoints, trivia attached by `provider_api::adapter::Attach`). The node kinds are the grammar's; a leaf of the old parser's tree is a token (`TypeName` and `Definition` are `Identifier` tokens, `<>` is one `Operator` token, `non-sealed` and `@interface` one `Keyword` token, a string or text block one `String` token); a switch group's statements are one flat `SwitchGroupStatements`, and a file without `package` is a flat `CompilationUnit` (only compact source files with methods outside a class get `UnnamedCompilationUnit` and `UnitBeforeFirstMethod`).
- Lossless: every byte is in a token or in trivia (an unknown character is an `Unknown` token and an error); `>` is always one token, `>>` and `>>>` are joined in an expression.
- Never panics; a syntax error is reported and parsing goes on at the next statement or member, every token stays in the tree. Left-recursive shapes (operator chains, member chains, array access, scoped names) are loops, the recursion that follows the source's nesting goes through `core_format::deep`: a chain of 10,000 calls, a sum of 10,000 terms and 5,000 `else if` parse.
- A declaration, a cast, a lambda and an expression start alike, so `look.rs` reads ahead over the tokens (where a type ends, what a `(` opens) before the parser commits.
- Parser comparison that led to it (the pinned corpus of 5,337 files, 48.8 MB, aarch64 Linux): rezel (a Lezer grammar port, one author, `0.0.0`) left 2 files with errors (an escaped `\"""` in a text block, panicking with debug assertions; a chain of about 300 calls), left `<`, `>`, `|`, `&` and `non-sealed` out of its leaves in 4,207 files, and parsed 11.8 MB/s with the adapter; tree-sitter-java had 14 files with errors (a type-use annotation on varargs) at 13.5 MB/s; cure-java-parser has no spans and aborts on deep nesting. The hand-written parser has 0 files with errors and parses 53 MB/s (`parse_speed` in `tests/parser.rs`, whole files from memory, release, best of 3). While rezel was still in the tree its trees and ours were identical on all corpus files but four (the two rezel bugs and two `char[] @Nullable []` types, which rezel nests differently).

Files: `*.java`.

## Formatting
The layout of google-java-format (Google style) in `crates/lang-java/src/format/` (`decl.rs` declarations, `stmt.rs` statements, `expr.rs` expressions, `space.rs` token spacing, `mod.rs` the shared helpers). It is one pass over the tree: every list (arguments, parameters, type parameters, binary operands, chain links, array initializers) is a group that fits on a line or breaks as google-java-format breaks it.
- Indentation is `indent_size` (2); a continuation is two levels, so AOSP style is `indent_size: 4` (no other option is needed). The width is `line_width` (100).
- Blank lines: the source's blank lines are kept (at most one) between members and statements; one is added around a method, around a type with a body and between the static and the other imports. A class body starts and ends without a blank line, except after a documented first enum constant.
- A parameter or argument list that does not fit moves below the opening parenthesis (one continuation), then goes one per line; a sole block lambda hugs the call; a `throws` clause moves to its own line when the parameters broke.
- A method chain is core-format's `call_chain` in the `fluent` style (options `member_chain_*` below): it fits on one line or breaks before every `.` with a continuation indent; a leading type name, `this` or a short name and the calls in `member_chain_head_calls` stay on the first line; a chain with fewer than two calls breaks as a whole expression.
- Binary operators break before the operator, all of one precedence together; a ternary breaks before `?` and `:`; an assignment or a declarator moves its right side below `=` first.
- A line comment keeps its line: when it follows an operator, `else`, `->`, `=` or a comma, what comes next starts a new line below it (so the output is a fixed point); after `;`, `)`, `]`, `}` or just before a closing bracket it moves to the end of the line without breaking the group.

Imports are sorted before the layout (`imports.rs`, a source rewrite): static first, then the others, ASCII order, no duplicates, one blank line between the groups; a block with a comment between imports is left as written.

Not done (named gaps in `tests/parity/google-java-format/differences.yaml`): re-wrapping long comments and `//x` to `// x`, dropping `()` from `@A()`, the extra indent of a chain inside a ternary condition. Javadoc, unused imports and long strings are kept as written (the flags of the parity run: `--skip-javadoc-formatting --skip-removing-unused-imports --skip-reflowing-long-strings`).

## Parity
`scripts/parity.sh format google-java-format` (google-java-format 2.0.0, its native binary: no JVM needed) formats the corpus of `tests/conformance/fetch-java-corpus.sh` (Guava, Gson, Error Prone, Commons Lang and google-java-format's own sources: 5,337 files) and `tests/parity/google-java-format/` with both tools. aarch64 Linux: identical on 4,504 of 5,337 files (84.4%; Guava, Gson and Error Prone are formatted with it already, Commons Lang is not); 0 files change on a second format; 0 are refused; the ratchet is `real-world` in `differences.yaml`. The corpus run takes about 40 s.

## Lint
Checkstyle is the reference (catalog: [../parity/checkstyle.yaml](../parity/checkstyle.yaml)). Pattern files in `crates/lang-java/rules/`: `empty-catch-block`, `string-literal-equality`, `boolean-literal-comparison`, `equals-hash-code`, `missing-switch-default`, `empty-statement`, `upper-ell` (warn by default) and `array-type-style`, `avoid-star-import`, `need-braces` (off). Rust rules in `src/rules.rs`: `unused-import` (warn), and `naming-convention` and `modifier-order` (off). `naming-convention` takes a regular expression per kind of name (`packages`, `types`, `methods`, `members`, `constants`, `parameters`, `catch_parameters`, `locals`; empty: not checked). Shared: `max-line-length` and the comment rules. The rest of Checkstyle's checks are `planned` in the catalog.

The preset `java/google` (`extends: [preset:java/google@1]`) sets 2-column indent, width 100 and turns on the style checks of `google_checks.xml` that exist (`array-type-style`, `avoid-star-import`, `modifier-order`, `need-braces`, `max-line-length`, `naming-convention` with Google's patterns). AOSP style is `indent_size: 4`.

## Options
<!-- generated:options -->
| Option | Values | Default | Description |
|---|---|---|---|
| `indent_size` | int 1-16 | `2` | Columns per indentation level (also the width of a tab). |
| `line_width` | int 20-1000 | `100` | Preferred maximum line width in columns. |
| `indent_style` | `space`, `tab` | `space` | Indent with spaces or tabs. |
| `final_newline` | bool | `true` | End the file with a newline. |
| `end_of_line` | `auto`, `lf`, `crlf` | `auto` | Line break style of the output: `lf`, `crlf`, or `auto` to keep the file's first line break. |
| `member_chain_min_calls` | int 1-1000 | `2` | A chain of member accesses and calls with at least this many calls is broken one call per line when it does not fit on a line (`fluent` style). |
| `member_chain_keep_accesses` | `never`, `head`, `always` | `head` | In a broken member chain an access stays on the line of the call after it (`.a.b()` is one group) instead of starting a line of its own: `always`, or `head` in the first group only (which stays on the line of the head when the head name is short or a type name). |
| `member_chain_break_on_block` | `never`, `inner`, `any` | `any` | Break a member chain one call per line when it has an argument with a block body (a lambda or an anonymous class) even if the chain would fit: `inner` when a call other than the last has one, `any` when any call has one. |
| `member_chain_head_calls` | list of words | `[stream]` | Calls that stay on the line of the head of a broken member chain when they are among its first two groups (`list.stream()`). |
| `member_chain_short_head` | int 0-100 | `4` | A name of at most this many characters at the head of a broken member chain stays on the line of the first call (`x.foo()`); a capitalized name, `this`, `base`, `super` and a keyword type always do. |
| `member_chain_indent` | int 1-4 | `2` | Levels of indentation of the lines of a broken member chain. |
<!-- /generated -->
