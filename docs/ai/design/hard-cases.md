# Hard cases

Hard cases left open by finished unification steps ([unification.md](unification.md), row H1). Each needs a prototype first. A case measured once and still open is recorded here with its numbers and its missing capability, then left.

### Python: `class A():` to `class A:`

Python `class A():` to `class A:` (the `Arguments` node vanishes, the safety check refuses 27 corpus files unless `Arguments` becomes `#optional`)

### Python: formatter constructs on the kit (binary chain, `maybe_parenthesize`, member chain, conditional, blank lines between members, comment rewrite)

Python binary chain, `maybe_parenthesize`, conditional, blank lines between members and comment rewrite on the kit (the lists conversion showed each needs a shared knob for a ruff quirk, e.g. the `OPEN` thread-local; prototype each against the 6,273-file corpus with `judge.py` before building; the member chain is done, see F8). Sizes read from the code against the shared shapes: binary chain, ruff lays out `BinOp`, `BoolOp` and `Compare` as n-ary operand lists where `shapes::binary_chain` follows one `left operator right` link, so it needs an n-ary spine, multi-token operators (`not in`, `is not`) and hooks for the comment after an operator (it stays with the operand before), the group per operator and operand after a multi-line string, the held comment of a nested operand and the `OPEN` break: about +120 shared / -190 Python; the can-omit-parentheses visit of `paren.rs` (about 150 lines of operator levels and bracket ends) reads the same spine; conditional, ruff prints `body if test else orelse` where `shapes::conditional` prints the test first, a layout of its own for 25 Python lines; blank lines, ruff needs exact counts (two after a definition at the top level, one nested, none before an overload stub) and a blank-line row says only that one is required, so rows need a count and conditions on stubs and first-in-block that only Python has, which the two-language rule refuses; comment rewrite, the `[rewrite]` rows of `core_format::rewrite` have no token condition for an empty `Arguments` pair, and `# x` spacing is a text rewrite of a trivia token

### HTML: `print/embed.rs`

HTML `print/embed.rs` (the printer lays embedded code out as `Doc`s of width-aware placements per element kind: `<script>` lines, attribute `expand`/hug/`explode_object`, Vue and Angular expressions; `lang-html` is the only host with `Opts::embed`, every other host splices text with a prefix in `core_incremental::embedded`, so no second user for a shared Doc-level path (measured: the text helpers moved to `core_format::embed`, -60 HTML lines; what is left is 620 lines of per-element placement over `model::N`, which no other host has); its `script_language` and `style_language` tables become `embed` rows when the printer reads the tree's elements instead of `model::N`)

### C#: `suppression-justification`

A pattern row (`kind: Attribute`, a `not-has` of a `NameEquals` `Justification` or a `has` of an empty or `<Pending>` value) reproduces the rule with one message and the attribute name as span, and passes the C# fixtures, but the decision (a suppression needs a reason that says something) is a mechanism any language with a reason on a suppression shares (Kotlin `@Suppress`, ESLint directive descriptions). Missing capability: an `#annotation` role with `name`, `arguments` and named arguments (`#key-value`) on C# `Attribute` (an `#opaque` node today) and Java/Kotlin annotations, then a `suppress` family check with rows for the attribute names, the argument name and the empty values. The Rust rule stays in `declarations.rs`.

### Unused suppressions: ruff RUF100 and ESLint unused directives

A suppression comment that silences nothing needs three things Sudsy has none of: (1) the tool's own codes at run time (`# noqa: F401` names ruff codes, the rules carry Sudsy ids; the code to id mapping lives only in `docs/ai/parity/ruff.yaml`, so every rule row would get a `codes` key, 125 rows for ruff), (2) a hook in `engine::silence` that returns which directive silenced which diagnostic, so the unused ones become findings of a shared `unused-suppression` rule, (3) `noqa` and `eslint-disable` read as suppressions with an optional code list (today only the `sudsy-*` markers suppress, the foreign `directive` rows only feed `unknown-rule-in-directive`). Measured with ruff 0.17.4 on the ruff parity corpus (`ruff check --isolated --ignore-noqa --select <the catalog's mapped codes>,RUF100` on the 1,666 files of the ruff parity work folder; 56 files have findings): 136 RUF100 findings, of them 58 `Unused blanket noqa` (no selected rule fires on the line: reachable with (2) and (3) alone), 46 `non-enabled` (32 are `E501`, not mapped: reachable with the code table), 32 `unused` (`F401` 20, `E741` 6: need (1) to (3)). Sudsy's own rules would agree on the line for the 32 `unused` and 58 blanket findings only as far as the mapped rules do (ruff coverage 95.6%). ESLint reports unused directives with no rule id (`reportUnusedDisableDirectives`), so the parity run drops them and the catalog has no entry: nothing to ratchet. Not built: a `codes` row in 125 rule files plus an engine hook is a step of its own, and a native `unused-suppression` for the `sudsy-*` markers alone moves no parity number.

### Rewrites left in language code

rewrites left in language code (each needs a `format: true` rule with a section used by a second language, and moving it onto Kotlin or Java changes their output, so it needs the ktlint or google-java-format ratchet run):

### JS and Kotlin: `format/rewrite.rs`, `ts/parens.rs`, Kotlin rewrites

JS `format/rewrite.rs` and `ts/parens.rs` (`;` and `new Foo()` are `[rewrite]` rows now, shared with PHP and, for `;`, Java; left: redundant and required parentheses, arrow parameters, comment moves before `{` and `;`, `quote_props`; second languages: Kotlin `no-unit-return` style parentheses, Kotlin `;` (ktlint `no-semi` drops one at the end of a line: a row cannot say that), Kotlin `rewrite.rs` (enum trailing comma, expression bodies, braces, blank before KDoc; second languages: Java and C# `curly`, Kotlin and Java trailing commas)

### Protobuf, Svelte, HTML: `sort.rs` and `rewrite_source` (sorted sections)

Protobuf `sort.rs` and Svelte root order and attribute shorthand in HTML `rewrite_source` (`sorted-sections` over groups with rank and key: L8's `sorted-imports` is the second user)

### Ruby: `skip_unused`

Ruby `skip_unused` (parameters left alone: needs a shared check over roles `#yield`, `#raise` and call names, with a second language)

### Single-use `scope.rows` facts

single-use `scope.rows` facts (`hosts`, `conventions`, `labels`, `methods`, `blocks`, `redeclared-in`, `parameter-properties`, `markup`, `interpolated`, `unused-prefix`, `globals`, `top-unchecked`, `ordered`, `globals-if`: one language each; become `lang.yaml` options or find a second user)

### HCL and PowerShell: token-line kit, PowerShell `place_breaks` and blocks

F13 leftovers: HCL and PowerShell run on the shared token-line kit (done: `core_format::frames` indents by open and continuation frames over the `[lines]` rows `open` and `close`; `columns::runs` and `columns::pads` line up cells; the spacing is `[space]` and `[at_least_one]` rows; HCL's `Punct` is split into `Open`, `Close`, `QuoteOpen` and `QuoteClose`; Rust src hcl 405 to 254 lines, PowerShell 530 to 489; terraform ratchet 312/318 unchanged; the PowerShell ratchet needs `pwsh`, not run here: corpus lossless and idempotent, 210 files); open: PowerShell `place_breaks`, `statement_block`, `in_block` and `placed_block` (about 130 lines: a block is found by the keyword or the closing parenthesis before its `{`, and the braces of a multi-line block are placed; needs a `[layout]` row for a block with a head and a rule on the source's lines; not prototyped)

### PowerShell: `blank` and `$x.ForEach{`

the `$x.ForEach{` condition of PowerShell's `blank` (a word that contains `.` or `::`: one language, so Rust)

### PowerShell: backtick continuation

backtick continuation on `shapes::continuation` (an `end_line` variant that keeps the source gap and nests; not prototyped: the shared `Frames` continuation frame is the form today)

### XML: tag, attribute and mixed-content layout

XML (tag, attribute and mixed-content layout: mixed content is kept as written, so `shapes::fill_children` has no use there until a reflow mode (`proseWrap`-like option, also for Markdown prose and YAML scalars) is built; `markup-whitespace-model` stays LOCAL for it)

### GraphQL: `ListValue`

GraphQL `ListValue` (done: the grammar wraps scalar items in a `Value` node and `ListValue` runs on `delimited_list`)

### Protobuf: message literal fields

Protobuf message literal fields (name, colon and value are flat tokens of `BlockLit`: open gap by the user's decision: the grammar does not expose `block_field` (hidden `_block_field` in tree-sitter-proto 0.6; a fork would add it, then `BlockField` and a `key_value` row with a default separator, about 130 lines of `lang-protobuf/format.rs`), and the local code stays until Protobuf has other parser needs; no fork)

### Shell: `continued_before`

shell `continued_before` (a trivia query: `continuation::precedes(token, escape)`)

### XML: `no-duplicate-include` on `unique-names`

XML `no-duplicate-include` on the `unique-names` family (measured after the compiled slot-path plan, the `Fx` hasher, the shared first step and one-pass messages: the rule costs 14.8M instructions against 7.4M for the local one on the 62.3M xml lint phase, +10.3%: per item about 850 against 220; what is left is the generic walk per item (kind tables, `Option` plumbing per step) and the second key-slot read per attribute; a row-specific scan would be the local rule again)

### Spacing leftovers (F2b): method

F2b spacing leftovers (measured: counts from the code, and ablation runs that turn the code off and read the ratchets; each with what the rows lack):

### CSS: `value.rs` `segment`

CSS `value.rs` `segment` (300 lines, 15 closures, 14 `gap(` reads, 12 `Dialect::` branches; no ablation: every value passes through it: `star`, `slash`, `sign`, `split_plus`, `spaced_sign`, `glued` and the like read a third neighbour on either side, the source gap between them (14 `gap(` reads) and the dialect (12 `Dialect::` branches), where a `[space]` row sees the pair of tokens, its parents and the texts of siblings, and has no dialect key; needs a `Flow` over value items with `keep` rows and a `dialect` condition fed from `lang.yaml`)

### CSS: `selector.rs` combinators and `nth`

CSS `selector.rs` combinators and `nth` (the same model)

### CSS: case sites in code

CSS case sites in code (units spelled by a table, hex hashes, at-rule, property, pseudo and keyframe names, `!important`: they depend on the position in the tree (property, prelude, flag) or on a part of a token (the unit of a dimension), so they need `[case]` rows with a parent condition and a token-part form; the second language for the part form is JS and Python `literal.rs` hex and exponent case)

### HTML: `dom.rs`/`model.rs` whitespace

HTML `dom.rs`/`model.rs` whitespace (inline text spacing follows the CSS `display` of the neighbours and is `shapes::fill`'s, not token spacing), and

### Shell: `arith.rs`

shell `arith.rs` (153 lines; ablation, `respace` off: 1 of 82 shfmt fixtures changes and the corpus ratchet is unchanged, so it moves only with a grammar that gives arithmetic nodes; no second language has arithmetic spacing, so rows would have one user: the tree holds `$(( ))` as one `Arith` token and the scanner lexes 34 operators itself; the rows need arithmetic tokens from the grammar: tree-sitter-bash has the nodes, the mapping and the `for ((;;))` form do not use them yet; about 10 rows would replace the `bin`/unary logic)

### SQL: F12b `select_targets`, `ctes`

SQL F12b (`select_targets`, `ctes`): the layout pipeline is a loop over points (`space`, `indent`, `breaks`, `wrap`, `breaks::ctes`, `select_targets`) whose result becomes a `Doc` only at the end (`print`), so a clause cannot be a `delimited_list` while `indent` and `wrap` still work on points; the rules themselves are width and comment conditions, which `ListStyle` (`force_break`, `break_from`, `AsSource`) can state, so the missing capability is the SQL clause tree as `Doc` input for `indent` and `wrap` (F12d), not a list style; no per-clause prototype is possible before that.

### SQL: F12d `indent.rs`, `wrap.rs`

F12d (measured, not built): the passes were run in reduced forms and the sqlfluff ratchet read (matching files, base in brackets; ansi, mysql, postgres, tsql, postgres-regress). `indent.rs` replaced by a plain bracket-balance indent (no untaken-indent, comment-line or imbalance logic): 109 (203), 153 (208), 73 (122), 49 (108), 8 (37), second format 3 and 2 files on regress and tsql; so the untaken-indent logic is load-bearing everywhere and cannot be dropped for a plain `Doc` indent. One layout round instead of the fixed point: 182, 199, 108, 83, 11, second format 23, 18, 36, 61, 151 files; so the loop is load-bearing too, and most of what it does is wrap and breaks reading the indent the previous round set. Missing `Doc` capability: sqlfluff's layout reads the source lines, not the tree alone. `wrap.rs` picks, per over-long source line, the breaks by indent balance and operator precedence among the points that line holds, skips indent points the line reaches before the width (`purge_before`) and takes the indent of the line from the source line; `indent.rs` keeps an indent untaken when its line did not break and reconciles it with the next break. A `Doc` group decides fit by its own content from the tree, with no per-point priority across a flat run of operators and no indent that exists only when a break follows within the source line. Needed before F12d can be tried: a `Doc` input where a break point carries a priority key and an indent that applies only if the point breaks (`indent_if_broken`) and `group`s that fit by the source line (`AsSource` at every point), then the clause tree can state `select_targets` and `ctes` as `delimited_list` styles; with only the existing kit the first prototype would reimplement `matched` and `reindent` over `Doc`, which is the same code moved.

### SQL: F12c MySQL block indents

F12c: the MySQL `BEGIN`/`LOOP`/`REPEAT`/`WHILE .. DO`/`UNTIL`/`END` scan is `core_format::block_frames` (a shared scan of flat statements by keyword, the words are a table in `blocks.rs`)

### String literals (F1b/F1c rest)

Left on hand-written string reading, measured once. Ruby: strings are `Quote` and `StringPart` tokens, never one token, so a `string` row has no token to sit on; `interpolation_check` and the regexp capture scan are 45 lines of lint on text that decode no escape. HTML expression scanners (`angular.rs` `string_step`, `svelte.rs`, `template.rs`): one-byte steps that carry the open quote between calls, and template literals nest; the JS row's `scan` takes a whole string, so each needs its own restructuring for about 25 lines. TOML `validate::string` (55 lines): `check_escape` replaces about 20 of them; the rest is the three-quote run rule, the scalar-value check and the blanks-then-break rule of multi-line strings, which only TOML has. Shell `arith.rs` quote scan: 3 lines over a `char` vector. The `rows()` accessor of the Java and Kotlin formatters (3 lines each): a default method that returns the formatter's `LazyRules` is the same 3 lines, and a `format.rules` text in `ParserSpec` would turn on the rows of every language that has the file, which changes their output. The per-language `string_value` wrappers (JS `literal.rs`, Python `util.rs`) and a callee-to-row map per grammar for `regex-call` (js.lint instr +3.00%: the callee text is built per call) stay open.


### PowerShell parity wrappers (X1)

`scripts/parity_psscriptanalyzer.ps1` (rules and run modes, JSON from `Invoke-ScriptAnalyzer`) and `scripts/parity_invoke_formatter.ps1` (CodeFormatting settings, BOM and blank-line handling) stay. A PowerShell module has no command line: its run is PowerShell text, so a `command` entry would hold the same lines as an inline `-Command` string with no gain. The agent host (linux aarch64) has no `pwsh`, `dotnet tool install PowerShell` is not a tool package and the release download was denied, so no prototype could run and both parity runs are unmeasured here. Missing: a host with `pwsh` to run `scripts/parity.sh psscriptanalyzer`, `format invoke-formatter` and `coverage psscriptanalyzer` before and after.
