# Pattern rules (`core-patterns`)

ast-grep-style YAML rule files; listed by `sudsy rules`, configured under `lint.rules`, suppressed, fixed and shown in the editor like any built-in rule. Why YAML: C# teams write everyday rules without a Rust build; the matcher is shared with `plugin-sdk`.

```yaml
# sudsy.yaml
plugins:
  patterns: ["rules/*.yaml"]      # globs relative to this file
```

A rule file holds one rule, or several separated by `---`:

```yaml
id: no-console-writeline            # plain id, or team/no-console-writeline
language: csharp                    # csharp | json | yaml
message: Log with the logger, not Console.WriteLine($$$ARGS)
severity: warn                      # off | warn | error: the level when the config is silent (default warn)
category: suspicious                # correctness | suspicious | style (default) | cleanup | performance
rule:
  pattern: Console.WriteLine($$$ARGS)
  not-inside:
    kind: MethodDeclaration
    has: { pattern: Main }
constraints:
  ARGS: { regex: '^"' }             # regex | not-regex | kind, on what a metavariable captured
fix: _logger.LogInformation($$$ARGS)
fix-title: Use the logger           # optional
fix-kind: unsafe                    # safe | unsafe (default: a template is not known to preserve meaning) | suggestion
```
Also `description` and `example`. Unknown keys are errors.

## Span, fix target, options
```yaml
id: no-empty-attribute-parens
language: csharp
message: the attribute has empty parentheses
rule:
  kind: AttributeArgumentList
  regex: '^\(\s*\)$'
report-on: first-token           # the diagnostic points at the `(`
# report-on: field NAME          # or the node in a role field of the match (parentheses looked through)
fix: ""
fix-trim-before: true            # the fix also takes the spaces before the parentheses
fix-kind: safe
```
- `report-on` sets the diagnostic span: `$NAME` (what the metavariable captured), `first-token` (the first token of the matched node), `first-token $NAME`, or `start` / `start $NAME` (an empty span at the start of the node, trivia included, or of the capture). Default: the matched node.
- `fix-on: $NAME` makes the fix replace what the metavariable captured instead of the matched node; `fix-trim-before: true` extends the replaced range back over the spaces and tabs before it. `fix: ""` deletes. A hit that did not capture a metavariable the `fix` names (a `regex` group in an optional part, as shell's `no-backticks`, which fixes only a body without a backslash) is reported without the fix. Both need a `fix`; `$NAME` must be captured by a pattern outside `has`/`child`/`not`.
- `options` declares typed options with a default, validated and shown in the schema and `sudsy rules <id>` like a Rust rule's: `{ name, type, default, choices, description }`, `type` being `bool`, `int`, `string`, `list` (of strings) or `regex` (a string that must compile); `choices` limits a `string` or each item of a `list`. A user sets them under the rule in `lint.rules` (`options:`).
- A constraint can read an option: `in-option: NAME` (the captured text is an item of the `list` option, or equals the `string` option) and `regex-option: NAME` (it matches the `regex` option, compiled once per value).

```yaml
options:
  - { name: banned, type: list, default: [Thread.Sleep], description: Calls that are reported }
rule: { pattern: $CALL($$$ARGS) }
constraints:
  CALL: { in-option: banned }
```

## Tokens
`rule` matches nodes; `token:` makes a rule report children instead: each own child of the matched node (a direct child, token or node) that has one of the listed kinds and satisfies `regex` / `not-regex` (on its text) is a diagnostic at that child, with the rule's `message`. `needs: "chars"` skips a node whose text has none of the characters (cheaper than a `regex` on the node). `skip-first: <kind>` leaves out the first own child of that kind (a command's name). `token` needs a `message` and goes without `layout`; `report-on`, `fix`, `fix-on` work per reported child (default span and fix target: the child). A hit captures `$0` (the child) and the named groups of `regex` (`(?<NAME>...)` is `$NAME`), usable in `message`, `fix`, `report-on` and `fix-on`. `deep: true` reads every descendant of the matched node instead of its own children (a rule on `Document` that looks at every comment). A `regex` / `not-regex` here and in `constraints` may hold `${NAME}`, a capture of the rule (written escaped; see Templates), and is built for each match.

```yaml
id: quote-at-expansion
language: shell
message: quote `$@` so the elements are not split again
rule:
  any: [{ kind: Command }, { kind: For }]
token: { kind: [Word, Concat], needs: '$', regex: '^[^"''`\\(]*\$@' }
```
Rules without `token` pay nothing for it.

## Layout
`layout:` asks the formatter for a layout; each entry is `<directive> [argument] [on $NAME]`:

```yaml
# rules/service-chains.yaml: every call of a builder chain on `services` on its own line
id: services-chain
language: csharp
layout: [break-all]
rule:
  kind: InvocationExpression
  regex: '^services\s*\.'          # the node's text changes once it is broken: allow whitespace
---
id: items-one-per-line
language: json
layout:
  - break-all on $V                 # a metavariable: the node it captured
  - blank-line-before 1
rule:
  pattern: '"items": $V'
```
- Directives: `break-all`, `keep-together`, `break-before`, `break-after`, `blank-line-before N`, `align`, `preserve`, `style NAME` (what each does and where it is honored: [../formatting/engine.md](../formatting/engine.md)). The target is the matched node, or with `on $NAME` the single node that metavariable captured (a token or a run captures no node: nothing is attached). `$NAME` must be captured by a pattern outside `has`/`child`/`not`.
- Without `message` the rule is layout-only: no diagnostic, not run when linting, `fix` not allowed; with `message` it reports and attaches. A layout-only rule has an id like any other: `sudsy rules` lists it, `lint.rules` sets its level (`off` attaches nothing), `sudsy-disable-next-line <id>` silences the node on the next line.
- Layout rules see syntax only. Two rules asking for conflicting layouts of one node (`break-all` + `keep-together`, ...) make that file fail to format, naming both. A bad directive is a config error naming the file.

## Matching
- Cost bound: a rule gets 1,000,000 match steps per node (its `inside`/`has` walks included), because several `$$$` in one pattern are O(n^k). Past that the rule reports "the pattern took too many steps" once and is off for the rest of that file.
- A pattern is a snippet of the language, parsed by its own provider. Metavariables are first written as identifiers (`__SUDSY_S_X__`, `__SUDSY_M_X__`), or as strings when that does not parse (JSON). The snippet is tried as is, with a `;` after it, and inside `{ }` (a JSON member); the node spanning all of it is the pattern.
- `$X` matches one node or token; `$$$ARGS` any run of siblings (also none, separators included, by backtracking); the same name twice must capture equal text; `$_X` captures nothing. In statement position write `$$$BODY;`.
- `rule` keys: `pattern`, `kind` (node kinds of the language's grammar), `regex` (node text without trivia), `path` (regex on the file path, `/` as separator: `'\.nomad$'`), `nth: N` (the node is the Nth, from 1, of its parent's children of its kind: `{ kind: EnumField, nth: 1 }`), `inside`, `not-inside`, `has`, `not-has`, `parent`, `child` (nested rules), `option`, `all`, `any`, `not`. Keys at one level all must hold. `has`, `child` and the negations drop their captures; `inside` (the nearest ancestor that holds) and `parent` keep them, so a node can use a name from its surroundings. A `regex` captures its named groups (`(?<NAME>\w+)`; unnamed groups capture nothing) like a pattern its metavariables:

```yaml
rule:
  kind: MessageType
  parent: { kind: Rpc, regex: '^rpc\s+(?<RPC>\w+)' }
```
- `parent` (the direct parent holds the rule) and `child` (some direct child does) are the one-level forms of `inside` and `has`, which look at every level. `option: NAME` holds when that `bool` option is true; `in-option: NAME` holds when the text of the node (or token) is an item of that `list` option. `regex-option: NAME` holds when the text of the node matches that `regex` option (compiled once per value). `rethrows: true` holds on a `#catch` whose body (nested `try`, functions and classes left out) has a `#throw` with no `value`, or whose `value` or `cause` is the catch's `binding`. Write a negation with `not`:

```yaml
rule:
  kind: CatchClause
  child: { kind: Block, regex: '^\{\s*\}$' }          # its own block, not one in a nested try
---
rule:
  kind: ParameterList
  parent: { kind: AnonymousMethodExpression }         # not the list of a local function inside one
---
rule:
  kind: FlowMapping
  not: { option: allow_empty, regex: '^\{\s*\}$' }    # `{}` is fine when allow_empty is on
```
- A rule must pin where it starts: a `pattern` or `kind` at the top (or in every `any` branch). `kinds()` is that root kind, so the engine calls the rule for those kinds only. A rule that pins none (regex or relation alone) is a config error: it would cost 1.6 ms per file per rule.
- A `regex` (also in `constraints`) is the Rust `regex` syntax with Unicode on. Its automata are built when the rule first runs, and ASCII text is matched by the same pattern compiled without Unicode, which answers the same on ASCII (`\s`, `\w`, ``, `.` and `[^x]` are far cheaper to build that way); text with other characters, and a pattern that needs Unicode (`\p{L}`), use the Unicode form. For ASCII text only, `[ 	]`-style classes and `(?-u:)` change nothing. Both forms are built with `regex-automata` directly and its full DFA off: `fancy-regex` (a dependency of the SQL parser) enables that engine for every regex of the build, which costs construction time, and `regex` 1.13 then misses matches of some patterns compiled without Unicode (`(?:^|\s)\([^()]*\)\[[^\]]*\]` on `(a)[b] x`).
- `message` and `fix` substitute captured source text. The fix replaces the matched node; fix loop and formatter do the rest.
- Templates (`message`, `fix`, and the `regex` of a constraint or of `token`) also take `${NAME}` (for a name followed by letters: `${RPC}Request`) and `${NAME:upper-snake}`, the capture in UPPER_SNAKE_CASE (`HTTPRequest` is `HTTP_REQUEST`). In a regex the text is escaped. A name must be captured outside `has`/`child`/`not`.

```yaml
rule: { kind: EnumField, inside: { kind: Enum, regex: '^enum\s+(?<NAME>\w+)' } }
token: { kind: [Ident], not-regex: '^${NAME:upper-snake}_' }
message: 'the value `$0` should start with `${NAME:upper-snake}_`'
```
- `type_matches` is not available (see roadmap).

## Roles
A rule can match on what a node is, not on the grammar's kind names, so one file runs in every grammar that has the roles ([../architecture/syntax-tree.md](../architecture/syntax-tree.md)):
- `role: <name>` or a list: the node has one of these role tags. It pins the kinds the rule runs on; a built-in rule whose top-level `role` no node of the language has is left out of that language.
- `operator: <kind>` or a list: the node's `operator` field is an operator of that kind (`Grammar::operator_kind`: `equality`, `comparison`, ...).
- `fields: { <field>: <rule> }`: the child in each named role field (`Grammar::slot`) satisfies the rule. A token child satisfies `regex` (its text), `role` (a token carrying the tag) and `all`/`any`/`not`.
- `items: { <field>: <rule> }`: some child in each named role field satisfies the rule (`fields` reads the first child; a catch lists several types). `single: <field>`: the role field holds exactly one child.
- `same: [a, b]`: the two fields have the same tokens. `empty: true`: the node has no child node, no token but its brackets and no comment before its closing bracket; `bare: true`: the same, a comment aside. `inner: <rule>`: the node, looked through parentheses and wrappers that hold nothing else, satisfies the rule.

```yaml
id: no-self-compare
rule:
  role: binary
  operator: [equality, comparison]
  same: [left, right]
```
Shared rule files live in `crates/core-patterns/shared/` and are built into every grammar that declares `roles` (`build/lang-crate.rs`; a `per-language` one only when the grammar has its `rule` row); their ids are `core_patterns::SHARED_IDS`, exempt from the one-fixture-folder-per-rule check, which a language that tests them meets with a `shared-bugs` folder. They are the bug rules `no-self-compare`, `no-constant-condition`, `no-constant-comparison`, `no-assignment-in-condition`, `no-self-assign` (a plain or logical assignment whose sides are the same tokens), `no-empty-block` and `no-empty-catch` (a `#catch` whose body is empty, or only a `#empty-statement`; with `broad_only` only a catch-all or one of the `types`; with `comment_handles` false a comment does not count), the rules on a grammar's tags `boolean-literal-comparison` and `no-wildcard-imports` (an `#import` ending in `*`), and the rules a language switches on with a `rule` row (below); `no-duplicate-case` (roles `#switch` and `#case`) and `no-duplicate-keys` (`#mapping`; `RuleHooks::has_accessors` lets a getter and a setter share a name, `flat_keys` serves a dict without entry nodes) are shared Rust rules.

### `rule` rows and `per-language`
A shared file with `per-language: true` exists in a language only when its grammar has a `rule` row for it; any shared rule can have a row. The row `rule <id> [level off|warn|error] [<option> "<value>"...]` replaces the rule's default level and the default of each option it names (a `list` option takes all the strings, the other types the first), so the language's facts are data of its grammar: the banned names of `no-print`, `no-eval` and `no-debugger` (`calls`, the text of a call's `callee`), `no-throw-generic` (`types`, a `#throw` of the type or of a call of it) and `no-broad-catch` (`bare`: a catch with no type is broad; `types`; a clause that rethrows the error or has a call matching `handled` is left alone), `no-print` (`redirect` and `console`: an argument that sends the output elsewhere or back to the console), `no-debugger` (`imports`) are rows; `no-useless-expression` reports an `#expression-statement` whose expression carries the `#pure` mark (a name, a literal, a member access, an operator expression; set per kind in the grammar). A grammar line `case-insensitive` makes `in-option` compare ASCII names without case (PowerShell command names). A user or preset sets the same options under `lint.rules`.

A tool's rule ids (ESLint `no-empty`, Checkstyle `EmptyCatchBlock`, ruff `F601`) are rows of its catalog in `docs/ai/parity/` that name the shared rule. Not in the shared rules: Java (literals are tokens, conditions are `ParenthesizedExpression`: no `no-constant-condition`, `no-assignment-in-condition`), PHP `true` and Ruby `==` (a call, not `#binary`).

## Loading
`core-patterns` compiles a file into `PatternRule`, a plain `core_rules::Rule`. Patterns compile when the config loads (a C# pattern file starts the worker then); a bad pattern is a config error naming its file. Ids are used as written; an id equal to a built-in's or defined twice for a language is a config error. Rules are leaked once per distinct file text (`Rule::meta` returns `&'static`), so an unchanged reload costs nothing. Fixtures: `provider_api::testing::expect` / `fix_with`.

## Built-in pattern rules
A language crate ships rules as pattern files: `crates/lang-<language>/rules/*.yaml` (every language), with no `language:` key (the crate's language; a key naming another language is an error). Adding a file is the whole change: the crate's `build = "../../build/lang-crate.rs"` lists the directory for `include_str!` (`provider_api::pattern_files!()`), the provider hands the text to `provider_api::with_pattern_rules` with its Rust rules (`OutOfProcessProvider::with_pattern_rules`, `Spec::pattern_files`), and they are built-in rules in every respect (`sudsy rules`, the generated rule table and schema, levels, suppressions, fixes). They use the same format and features as a user's file; `severity` is the default level and a rule that is off by default says `severity: off`.

The YAML of a language is read the first time its provider's rules are asked for (`LazyRules` in `provider-api`: linting a file of the language, `sudsy rules`, the rule ids a config names, never `format`), once per process; the patterns compile when the rule first runs, through the provider's worker, once per process, and only for rules that are on. A run that never lints that language reads none of it and never starts its worker for them. Built-in files cannot have `layout:` (`format` consults `LanguageProvider::rust_rules` only). If the compile fails then (the worker is down), the rule is skipped for that file, the failure is not remembered (the next file tries again) and it is reported once per run through the same warning as an unavailable semantic model (`Rule::unavailable`). A broken file is a bug: `every_embedded_pattern_compiles` (`crates/lang-csharp/tests/builtin_patterns.rs`) compiles all of them. A rule that needs semantics, flow, project scope or logic a pattern cannot say stays in Rust; its fixtures under `tests/rules/<language>/` are the same either way.

### Families
A built-in file can instead name a family, Rust code that implements a group of similar rules, and give it `params`:

```yaml
id: doc-param-has-text
family: doc-element
params: { check: text, element: param, scope: element, skip-inheritdoc: true, report: element }
message: the <param> should have text
description: ...
example: ...
severity: off
category: style
requires: [types]      # optional, patterns too: layers the rule needs (`uses`: layers it improves with)
fix-kind: safe         # a family rule that fixes
```
The provider lists its families (`OutOfProcessProvider::with_families`; `Family` and `Params` are in `core-rules`); a family builds the rule from its metadata, the file's `message` and its params, and rejects unknown ones, which `every_embedded_pattern_compiles` catches. Everything else (id, severity, options, description) is the file's, so a family rule is a built-in rule in every respect. `rule`, `fix` and the other pattern keys do not go with `family`. Dockerfile families: `run-command` (a check on the commands of a `RUN`) and `once-per-stage` (an instruction that may appear once per stage); params in `crates/lang-dockerfile/src/rules/`. A tree-sitter language lists its families in `Spec::families` (`families:` in `lang.yaml`). Shared family (`core_rules::unique::FAMILY`, `unique-names`): params `container` and `item`, grammar tags; the name of each `item` below a `container` node (its children, or with `deep` what is below them, not entering an item or another container) must not repeat; `name` is a slot path (`core_rules::slot_path`: slots, kind names, `[key]`, `:first|last|all`, `|` alternatives; the slot `name` by default), `group` a second path that is part of the key, `fold: lower`, `strip` (characters trimmed off the name), `reset` (a name that starts with it and one that does not undo each other), `report: item` and `lead` (what the report covers), `related` (the note on the first occurrence); `message` has `{name}` and `{group}` (GraphQL `unique-operation-name`, `unique-fragment-name`; Dockerfile `duplicate-stage-name`, gitignore `no-duplicate-pattern`, Markdown `no-duplicate-heading`, SQL `duplicate-table-alias`). `core_rules::unique::Repeats` is the first-seen bookkeeping rules in Rust use for the same report. Shared family `simplify` (`core_rules::simplify`): `check` is `collapsible-if`, `needless-bool` (`return True`/`False` in each branch), `if-with-same-arms`, `boolean-conditional` (params `then` and `else`, `true`, `false` or `any`), `twisted-conditional`, `negated-comparison` (params `operator`, `except-in` names functions that are left alone, `except-guard`; fix), `double-negation` (fix), `redundant-cast` (`!!x` or a call of a name in `calls` where a boolean is expected; message `{name}`) `jump-outside` (param `jump`: words of `break`, `continue`, `return`, `yield`, `await`; `inside` lists tags besides `#loop` a `break` may be in; a `yield` is also outside in a comprehension), `identical-operands` (`a == a` on a `#binary`, or on a `#call` of an operator method on an `object` with one argument, Ruby; param `operators`; message `{name}`), `repeat-once` (params `method`, `max`, `with`: the fields that hold a block; Ruby `0.times { }`), `constant-first` (a comparison with a constant on the left and none on the right; params `calls` (functions whose call is a constant), `owners` and `members` (`int.MaxValue`); safe fix: swap the operands), `assert-false` (`#assert` with a `false` test), `nested-with` (`#with` whose only statement is a `#with`), `lonely-if` (an `if` that is the only statement of the last `else`) and `superfluous-else` (param `jump`: `return`, `throw`, `break` or `continue`; a last `else` after branches that all end in it); they read `#conditional` (`condition`, `then`, `else`, `#branch` clauses), `#unary`, `#binary`, `#call`, `#literal`, `#parenthesized`, `#return`, `#loop` (`body`), `#function`, `#assert` (`test`) and the marks `#with`, `#yield` and `#await`, and run in any grammar with those roles. The shared rule `unknown-rule-in-directive` reads `suppress directive "<marker>"` and `suppress directive-group "<group>"...` rows (Ruby: `rubocop:` and the departments). Shared family `names` (`core_rules::listed`): the language supplies a `Targets` table (what the names of each `target` are, read off its tree), the family checks them by `mode` (`allowed`, `disallowed`, `pattern`, `lower`, `unique`; CSS, HTML `id-pattern`, `class-pattern`, `name-pattern`, `attr-pattern`). Shared family `blank-line-before` (`core_rules::blank_lines`): params `tag` (the statement nodes), `kind` (a kind name), `block` (tag of a block node), `always` (`true` or `multi-line`), `except` (`first-nested`, `after-same-kind`, `after-blockless-same-name`), `ignore` (`after-comment`, `inside-single-line-block`), `skip` (first words left out, `*` suffix: prefix); reports a missing blank line, or one that is not wanted (the `except` cases); CSS `rule-empty-line-before`, `at-rule-empty-line-before`, `declaration-empty-line-before`. Shared family `max` (`core_rules::max`): option `max` against a count over grammar tags, `count: ancestors` (nesting), `depth` (nesting below an outermost node) or `lines` (of descendants tagged `of`), `tag`, optional `report` (tag of the node whose first token is reported); CSS `max-nesting-depth`, GraphQL `selection-set-depth`, Makefile `max-body-length`. Shared family `naming-convention` (`core_rules::naming`): every param but `message` and `match` is `<option>: <tag>[<parent-tag][&modifier...][=path][|...]` (`&word`: a modifier keyword the node, or its parent, has; `=path`: a slot path for the name, `name|key` by default) and the option (declared in the file, `PascalCase`, `camelCase`, `snake_case`, `UPPER_CASE` or `off`; with `match: regex` a regular expression, empty: not checked) styles the name of nodes with that tag; `message.<option>` is the message of one option; GraphQL, Java (Checkstyle's name checks). HTML families: `meta` and `template` (Vue template checks on attributes and tags: one directive's value, argument and modifiers, its element and its neighbours; a pattern rule per directive would compile a regex each, which the lint bench of the small `.vue` inputs counts in full; params in `crates/lang-html/src/template.rs`). C# families: `doc-element` (a documentation element must exist, have text or declare a name; params in `rules/doc_element.rs`) and `finding` (reports what a worker check found).
