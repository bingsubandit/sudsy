# Gradle (`lang-gradle`)

## Parser
tree-sitter through `core-treesitter` (mapping in `crates/lang-gradle/lang.yaml`, grammar `grammars/gradle.grammar`) over the fork `grammars/tree-sitter-groovy` of `dekobon-tree-sitter-groovy` 0.6.0 (Apache-2.0 OR MIT; changes and numbers in its `FORK.md`). Chosen on the 1,538-file capped corpus of [../design/gradle.md](../design/gradle.md) against the murtaza64, loganprice and amaanq grammars and a hand-written lexer: 89.3% of files error-free (the others 80.6%, 71.9%, 46.2%), 4 of 70 probes failing, the only one with an active upstream and a scanner for GStrings, slashy and dollar-slashy strings. The fork parses command expressions with several or named arguments after a name or dotted name (`dependsOn a, b`, `inputs.property "k", v`, `attributes 'k': v`, `def t = project.tasks.create "x", Jar`) and Java-style lambdas: 98.6% of the capped corpus and 99.4% of all 6,481 files (the 39 others are `<% %>` or `@x@` templates or rare forms). Speed 12.7 MB/s. A string is one token however many `${}` it holds; a file with an error node is not formatted. `*.gradle` only; `*.gradle.kts` is Kotlin.

## Formatting
Rows only (`format.rules`), written line by line: every line keeps its break and is indented by the brackets open in front of it (`frame-indent`), 4 spaces by default (`indent_size`, `indent_style`; a tab in the source is converted). Comments and strings are printed as written.
- No blank line after an opening bracket that ends a line or before a closing bracket that starts one (`trim-bracket-edges`); blank lines collapse to `max_blank_lines` (1); one line break ends the file.
- One blank after a comma or `;` and none before; none inside `(` `)` `[` `]`, one inside the braces of a one-line closure, one before `{`; one around `=` and the compound assignments, ternary `?` and `:`; one after the `:` of a named argument or map entry and none before; one after `if`, `for`, `while`, `switch`, `catch`, `synchronized`.
- Left as written: quotes, parentheses of a dependency declaration, blanks around binary operators, a trailing `;`, comment text, line width.
- The rows are the decision table of [../design/gradle.md](../design/gradle.md): pooled held-out agreement 87.1%; the indent width (4 against 2 in 41% of occurrences) is the one row under 80% and the reason for the option. No reference formatter runs; Spotless greclipse is the comparison.

## Lint
No reference tool; the leg follows [../development/no-reference-tool.md](../development/no-reference-tool.md) section 4: checklist 85.7% (21 entries: 13 of the Gradle upgrade guides and the Android plugin docs, 8 CodeNarc rules; 18 have a rule with its source link, an example in its doc and a fixture in `tests/rules/gradle/`, 3 are `planned`), precision 97.3% over the zero-config rules (judged sample: `tests/parity/gradle/precision.md`). Rules are pattern rules (`crates/lang-gradle/rules/*.yaml`) over the grammar's statement and expression nodes, plus the shared scope rules (`scope.rows`) and `unreachable-code`.
- On by default (removed or broken on current Gradle, or bug-like): `no-removed-configuration`, `no-jcenter`, `no-lint-options`, `no-legacy-archive-property`, `no-main-class-name`, `no-upload-archives`, `unreachable-code`, `no-gstring-map-key`, `no-empty-block`.
- `preset:gradle/modernize` (name a Gradle version or are taste): `no-apply-plugin`, `no-compile-sdk-version`, `no-build-tools-version`, `no-java-convention-compatibility`, `no-build-dir`, `no-floating-version`, `no-tasks-create`. `preset:gradle/codenarc`: the CodeNarc set (`unused-import`, `unused-variable`, ...).
- CodeNarc EmptyIfStatement is the shared `no-empty-block` (an empty `if`, `else`, loop or `try` block; an empty `catch` is not reported); `no-floating-version` is shared with XML (NuGet `PackageReference`): Gradle gives the `floating` pattern in its grammar's `rule` row, and reads a dependency notation at the start of a statement (`implementation 'a:b:1.+'`, `classpath("a:b:latest.release")`).
- `planned` (no rule yet): CodeNarc ConstantAssertExpression (the shared `no-constant-condition` on `#assert` once the Gradle grammar has an `assert` test field); GStringExpressionWithinString (a `literal-checks` kit row; a `${}` in single quotes is mostly text for Maven, spotless or a shell: 0 of 36 on the corpus, so the row needs a way to tell them apart); BitwiseOperatorInConditional (a new shared rule on the `binary` and `conditional` roles; no corpus finding).
- Corpus run (6,481 files, optimized dev build, 8 threads, both presets): 1.5 s.
- Named gaps: `no-removed-configuration` reports a file that defines its own `compile` configuration (4 of 50 judged); `no-build-dir` reports a local variable named `buildDir` (3 of 50); `unused-variable` covers `def x = value` only (the parser splits typed declarations); `minSdkVersion`/`targetSdkVersion`, `task x <<` and `http://` repositories have no source; no corpus finding for `no-empty-block`; `no-apply-plugin` has only the plugins page as source, not a deprecation text; Android lint and Nebula stay partial sources (they need a build).

## Conformance
The ignored test `conformance` of `crates/lang-gradle/tests/gradle.rs` runs over the 6,481 files of `tests/conformance/fetch-gradle-corpus.sh`: round trip, meaning and idempotency hold on every file that parses; 39 files do not parse (stable 99.4%, ratchet in `tests/parity/stability/differences.yaml`). Agreement 87.1% (`tests/parity/gradle/differences.yaml`).

## Options
<!-- generated:options -->
| Option | Values | Default | Description |
|---|---|---|---|
| `indent_size` | int 1-16 | `4` | Columns per indentation level (also the width of a tab). |
| `indent_style` | `space`, `tab` | `space` | Indent with spaces or tabs. |
| `final_newline` | bool | `true` | End the file with a newline. |
| `end_of_line` | `auto`, `lf`, `crlf` | `auto` | Line break style of the output: `lf`, `crlf`, or `auto` to keep the file's first line break. |
| `max_blank_lines` | int 0-10 | `1` | Largest number of blank lines kept between lines. |
<!-- /generated -->
