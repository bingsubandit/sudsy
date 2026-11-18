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
| `trailing_commas` | `never`, `multiline` | `multiline` | Trailing comma after the last argument or parameter of a call or declaration that is laid out on several lines: `multiline` adds one, `never` removes it. |
| `function_signature_wrap_min_parameters` | int 1-1000 | `2` | A function or constructor with at least this many parameters puts each on a line of its own, even when the signature fits. |
| `class_signature_wrap_min_parameters` | int 1-1000 | `1` | A class with at least this many primary-constructor parameters puts each on a line of its own, even when the signature fits. |
| `class_signature_wrap_min_supertypes` | int 1-1000 | `2` | A class or object with at least this many supertypes puts each on a line of its own. |
| `assignment_layout` | `compact`, `fluid`, `break` | `break` | Where the value after `=` (or `=>`) goes when the line is too long: `compact` moves a binary expression, a conditional or a query to the next line and leaves any other value on the line; `fluid` also moves strings, collection expressions and property initializers, and any other value whose first line does not fit after the operator (a string that spans lines stays on the operator's line; the body of an expression lambda moves below `=>`; chains `a = b = c` get layouts of their own); `break` moves every value that does not fit. |
| `assignment_conditional` | `always`, `binary_test` | `always` | A conditional (`a ? b : c`) as the value of an assignment moves below the operator when it does not fit: `always`, or `binary_test` only when its test is an operator chain. |
| `assignment_indent` | int 1-4 | `1` | Levels of indentation of a value that moves below the operator of an assignment. |
| `member_chain_style` | `fluent`, `grouped`, `wrapped` | `wrapped` | Layout of a chain of member accesses and calls that does not fit: `fluent` breaks before every dot once there are `member_chain_min_calls` calls; `grouped` keeps the head, its calls and indexes in the first group and puts a call group per line after it (a chain of up to two groups stays on one line); `wrapped` keeps the accesses in front of the first call with the head and puts every later dot on a line of its own, a call after a block or argument list that ends on a line of its own staying on that line. |
| `member_chain_min_calls` | int 1-1000 | `2` | A chain of member accesses and calls with at least this many calls is broken one call per line when it does not fit on a line (`fluent` style). |
| `member_chain_force_links` | int 0-1000 | `4` | A chain of member accesses and calls with at least this many dots always breaks (`wrapped` style; 0: never). |
| `member_chain_indent` | int 1-4 | `1` | Levels of indentation of the lines of a broken member chain. |
<!-- /generated -->

## Formatting
Layout is our own code over the lossless tree on the shared kit (`crates/lang-kotlin/src/format/`: `decl.rs` declarations, bodies and lists, `expr.rs` control flow, lambdas, operators, `space.rs` token spacing), following ktlint's `ktlint_official` style:
- every node is its children one after the other (`seq`) with the spacing of `space.rs`; a line break the source has stays a line break and everything after the first one hangs one level, except after modifiers and annotations and for a closing `)` or `]` on a line of its own;
- the kit lays out what has a shape: bodies, blocks and `when` (`shapes::block`), argument and parameter lists (`shapes::delimited_list`: a function with `function_signature_wrap_min_parameters` parameters or a class with `class_signature_wrap_min_parameters` breaks one per line, a call keeps the break the source has between arguments, trailing comma per `trailing_commas`), `=` of properties and functions (`shapes::assignment`, value below the `=` when the line is too long), strings as written;
- blank lines: before declarations of a file or class body (not between consecutive properties or type aliases unless the second has a comment; a blank between or after the comments in front satisfies it), after the package, the imports and the file annotations; none after `{` or before `}`; empty braces stay as written (`{}`, `{ }`, `{` newline `}`); a block comment is re-indented only when it is KDoc (`#indentable-doc`);
- `rewrite.rs` is ktlint's code rewriting, done on the source before formatting (`rewrite_source`, the result is parsed again and checked against): import order (ASCII, `java.`, `javax.`, `kotlin.` last, aliases last, no duplicates; not when comments are among the imports), a trailing comma after the last entry of an enum with several entries or one per line (`;` goes on its own line), `{ return x }` and `{ throw x }` as function body to `= x`, braces around the branches of an `if`/`else` chain or the entries of a `when` that have none when another one has them, spans lines or would go past the line width, and around the body of a loop on a line of its own, a blank line between a `//` comment and the KDoc after it.

Member chains are the shared `call_chain` in the `wrapped` style (`member_chain_*` options in `lang.yaml`): the accesses of a plain name path in front of the first call stay with the head (`kotlin.math.f()`), every later dot starts a line when the chain breaks (four dots or more, a break before a dot in the source, a lambda or argument list that breaks in a link other than the last, or a line that is too long), and a call after a `}` or `)` that ends a line stays on it (`}.toList()`). A class or object with `class_signature_wrap_min_supertypes` supertypes puts one per line (`shapes::heritage`). Class headers, member chains and `when` are `[layout]` rows of `crates/lang-kotlin/format.rules` (shapes `heritage`, `chain`, `cases`); the rest is Rust. Annotations of a declaration stand on a line of their own. A `when` with an entry that spans lines has a blank line between all entries, else none. A lambda, `=` of a property, an argument (`name = value`) or an assignment moves its value below the operator when it spans lines, except a lambda; a comment at the end of the line stays there.

Gaps against `scripts/parity.sh format ktlint` (named in [../design/shared-core.md](../design/shared-core.md#layout)), by what is left: ktlint rewrites that belong in a shared rewrite layer (`: Unit`, `;`, modifier order, empty `()` of a class, `//x` and `/* c */` comments, braces around an `if` without `else`, `: Unit` added when `{ throw x }` becomes an expression body), binary operator chains use source breaks (no `binary_chain`), a value that ktlint moves below `=` while Sudsy keeps the call on the line (and the reverse: ktlint breaks both), the width of a chain that ktlint wraps before it moves the value, comments between `)` and `{`, blank lines in lambdas, `string-template-indent`; files ktlint leaves alone because it fails on them (stdlib builtins) cannot match.


## Rules
Pattern files (`crates/lang-kotlin/rules/ktlint.yaml`) for the ktlint rules that are about names and structure rather than layout: `no-wildcard-imports` (on), `no-unit-return` (on), `no-empty-class-body`, `package-name`, `class-naming`, `enum-entry-name-case`, `no-empty-file` (off). The shared `max-line-length` and `comment-space` cover two more. `unused-import` (on; ktlint `no-unused-imports`) is the shared scope rule on `src/scope.rs`: an import declares the last name of its path or its alias, every `ReferenceExpression` and infix function name is a use, a name in KDoc is a use, and imports of operator, delegate and destructuring functions (`getValue`, `plus`, `component1`) are never reported. The catalog `docs/parity/ktlint.yaml` lists every ktlint rule with its status; the layout rules are the formatter's.

## Parity
`scripts/parity.sh format ktlint` (ktlint 1.8.0, `ktlint_official`, see [../development/parity.md](../development/parity.md)) over the 5,776 corpus files it can parse: the ratchet counts the files that match byte for byte (3,986 of 5,776, 69%; the rest differ by the gaps above); a second format changes no file and the safety check refuses none.

## Conformance
`bash tests/conformance/fetch-kotlin-corpus.sh`, then `cargo test --profile corpus -p lang-kotlin --test integration -- --ignored --nocapture conformance`: kotlinx.coroutines, ktor, nowinandroid, ktlint and the Kotlin standard library, `.kt` and `.kts` (6,632 files, 20 s in the `corpus` profile): every file parses without errors, prints back byte for byte and satisfies the trivia rule; the 322 compiler PSI parser test inputs (`testData/psi`) that hold deliberate errors are reported, not required to parse. An unclosed block comment is an error.
