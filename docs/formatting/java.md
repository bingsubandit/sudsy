# Java (`lang-java`)

## Parser
rezel-lang-java (Java up to 26: records, sealed types, switch patterns and guards, unnamed variables, text blocks, `import module`, flexible constructor bodies; an LR parser from a Lezer grammar, pinned `=0.0.0`) behind `crates/lang-java/src/adapter.rs`, which walks rezel's tree once and builds ours with `provider_api::adapter::replay` ([../architecture/adding-a-language.md](../architecture/adding-a-language.md)); the tree shape is `grammars/java.grammar` (one row per rezel node name, loose bodies; leaves are tokens). Lezer leaves out the tokens its grammar never names (`<`, `>`, `|`, `&`, `@interface`): the adapter reads them from the text between leaves. A string or text block is one `String` token, a comment one `Comment` token (`//`, `/* */`, `/** */`). Rezel's recovery points (`⚠`) are syntax errors.

Files: `*.java`.

## Formatting
The layout of google-java-format (Google style) in `crates/lang-java/src/format/` (`decl.rs` declarations, `stmt.rs` statements, `expr.rs` expressions, `space.rs` token spacing, `mod.rs` the shared helpers). It is one pass over the tree: every list (arguments, parameters, type parameters, binary operands, chain links, array initializers) is a group that fits on a line or breaks as google-java-format breaks it.
- Indentation is `indent_size` (2); a continuation is two levels, so AOSP style is `indent_size: 4` (no other option is needed). The width is `line_width` (100).
- Blank lines: the source's blank lines are kept (at most one) between members and statements; one is added around a method, around a type with a body and between the static and the other imports. A class body starts and ends without a blank line, except after a documented first enum constant.
- A parameter or argument list that does not fit moves below the opening parenthesis (one continuation), then goes one per line; a sole block lambda hugs the call; a `throws` clause moves to its own line when the parameters broke.
- A method chain of a name and calls fits on one line or breaks before every `.`, except a leading type name or `this` (`prefix_len`); a chain with fewer than two calls breaks as a whole expression.
- Binary operators break before the operator, all of one precedence together; a ternary breaks before `?` and `:`; an assignment or a declarator moves its right side below `=` first.
- A line comment keeps its line: when it follows an operator, `else`, `->`, `=` or a comma, what comes next starts a new line below it (so the output is a fixed point); after `;`, `)`, `]`, `}` or just before a closing bracket it moves to the end of the line without breaking the group.

Imports are sorted before the layout (`imports.rs`, a source rewrite): static first, then the others, ASCII order, no duplicates, one blank line between the groups; a block with a comment between imports is left as written.

Not done (named gaps in `tests/parity/google-java-format/differences.yaml`): re-wrapping long comments and `//x` to `// x`, dropping `()` from `@A()`, the extra indent of a chain inside a ternary condition. Javadoc, unused imports and long strings are kept as written (the flags of the parity run: `--skip-javadoc-formatting --skip-removing-unused-imports --skip-reflowing-long-strings`).

## Parity
`scripts/parity.sh format google-java-format` (google-java-format 2.0.0, its native binary: no JVM needed) formats the corpus of `tests/conformance/fetch-java-corpus.sh` (Guava, Gson, Error Prone, Commons Lang and google-java-format's own sources: 5,337 files) and `tests/parity/google-java-format/` with both tools. aarch64 Linux: identical on 4,503 of 5,337 files (84.4%; Guava, Gson and Error Prone are formatted with it already, Commons Lang is not); 0 files change on a second format; 0 are refused; the ratchet is `real-world` in `differences.yaml`. The corpus run takes about 40 s.

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
<!-- /generated -->
