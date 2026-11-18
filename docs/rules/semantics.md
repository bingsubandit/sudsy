# Semantic analysis

## Capability layers (`core-semantics`)
Providers declare layers, rules require them; JSON and YAML declare none. JavaScript, TypeScript and Python declare none either: their scope rules (`undefined-variable`, `unused-variable`, `F821`, `F841`, ...) read the model of `core-semantics::scopes`, which the language builds in process on the first of them that runs on a file and shares between them ([../formatting/js.md](../formatting/js.md#scope-layer), [../formatting/python.md](../formatting/python.md#scope-layer)); the layers below describe what a provider answers across a process boundary (C#).

| Layer | Answered in C# by |
|---|---|
| Binding | Roslyn (`unused_imports`) |
| Types | Roslyn (`declarations`: async, task-returning, implements, override flags) |
| Values | Roslyn (`maybe_null`) |
| Flow | Worker exports Roslyn control-flow graphs; analyses run in the core |
| Project | Roslyn references; project phase in the core |

`symbol_findings` (Types) is one walk over the file for the checks that need a symbol or type: `base` calls that bind the same as `this`, default value-type constructors, partly qualified usings in namespaces, `ValueTuple` spelled out, unnamed tuple elements in signatures, `Debug.Assert`/`Fail` messages, `inheritdoc` on members that inherit nothing. Each finding carries its span and the replacement text the fix uses; the rules in `symbol_checks.rs` only turn findings into diagnostics. Only the `Debug` message rules also run without the answers (by the written name).

Answers are file-wide enumerations (`Answers`, `Declaration`, `Reference`), not point queries. Symbol ids are Roslyn documentation-comment ids (`M:Shop.Orders.OrderService.GetRecentAsync(System.Int32)`). `fast` never touches the worker's model; a `standard` run with no semantic rule enabled asks for nothing.

## Flow (`core-flow`)
- Export: blocks of simple operations (local read/write/read-write/declare; calls with callee symbol id and attribute flags; one `Other` mark per root operation), edges (always/true/false), constant branch conditions. Every block of a `try` has an edge to each handler start. Locals captured by lambdas/local functions or address-taken are flagged and excluded from dead-store analysis; parameters are not tracked.
- Unreachable blocks stay in the export; the core recomputes reachability from edges, constants and summaries.
- Generic worklist solver (forward/backward, any lattice, edge hook). Analyses: reachability, liveness (dead stores), reaching definitions, definite assignment (the last two have tests but no rule; the compiler already reports CS0165).
- `unreachable-code` (the shared flow family, below) reports the first statement of a run, only when none of its operations can run (`return c ? throw e : 0;` is not reported). With C#'s flow layer it reads the Roslyn graph; a body Roslyn cannot build a graph for (unresolved symbols) is built from the roles like every other language.
- `build` (language neutral) makes a graph from a grammar's statement roles (`#block`, `#conditional`, `#loop` with `condition`/`iterable`/`init`/`update`, `#switch`/`#case`, `#try`/`#catch`, `#return`, `#throw`, `#break`, `#continue`, `#labeled`): one `Other` operation per statement (its code span); loops without a test or with a literal `true`/`True`/`1` test never end by themselves; a `finally` block is built once per way into it; calls, accesses, name reads and `throw`s of a `try` body lead to its `catch` blocks. `Flow::reach` answers which statements can never run (`Reached::is_dead`), whether the end of the body is reachable (`end`) and whether a `case` ends reachable (`case_ends`). A loop that has a `break` somewhere inside (outside nested functions) is never endless, so a statement the grammar gives no role cannot hide a way out; a loop with no test is endless only where its kind declares a `condition` field; a `#negated` loop (Ruby `until`) runs while its test is false. `#statements` kinds (an `else` or `finally` clause, `lock`) are read like a `#block` (`items`); a `switch` kind tagged `#fallthrough` runs each arm into the next, any other leaves the switch after each arm.

## Flow family (`core_rules::flow_rules`)
`unreachable-code` and `no-fallthrough` are shared rules over `core_flow::build`: a language gets them with no rule code when its grammar has the statement roles (`#break` is the sign) and, for `no-fallthrough`, a `#fallthrough` switch (JS, Java, PHP; Kotlin `when`, Python `match`, Ruby `case` and C# sections have none). `unreachable-code` is dispatched on the nodes that can leave code behind them (`#return`, `#throw`, `#break`, `#continue`, `#catch`, and a `#loop` that looks endless); the first one with a statement surely after it (a following statement of its list, up to an `if` without `else`, a loop or a handler) builds the graph of its unit once (the nearest `#function`, or the file; a provider body kind is a unit too, but with the semantic layer on the provider's flow layer reads it instead), so code without such a jump builds nothing. It reports the first statement of a run that the graph says never runs, except kinds tagged `#inert` (empty statement, hoisted function declaration, a C# `throw` or labeled statement) and `#inert-bare` ones none of whose declarators has an `init` (JS `var x;`, which hoists); a `catch` that no throw can reach is reported as a whole. It is on by default (`warn`), with an editor-only fix that deletes the statement. `no-fallthrough` reports a `case` whose end is reachable (`Flow::case_ends`) and that has statements, or is empty and followed by a blank line, unless a comment before the next `case` (or at the end of its block) says `falls through`. Not modeled yet: PHP `if`/`else` bodies and the file level (the wrappers have no `items`), Java arrow `switch` arms, Ruby `begin`/`rescue`, Python `with` and `try`/`else`, `goto`.

## Scope rules (`core_rules::scope_rules`)
One rule family on the binder of `core_semantics::scopes` for the languages that supply a table (`ScopeLang`: rows by node kind, the token kinds a name is written with, and options). The model is built by the first scope rule that runs on a file and shared with the others. A rule needs no code of its own in a language: it is one line in the language's rule list.

| Rule | Java | Kotlin | PHP | Reference |
|---|---|---|---|---|
| `unused-import` | on | on | | Checkstyle UnusedImports, ktlint no-unused-imports; an import is used when any name of the file is spelled like it (resolution does not count: a local does not hide an imported function), or a doc comment mentions it |
| `unused-local-variable` | on | | | Checkstyle UnusedLocalVariable: never read (assigned only counts as unused; `x++` reads) |
| `parameter-assignment` | off | | | Checkstyle ParameterAssignment |
| `undefined-variable` | | | on | PHPStan level 0 |

Rule rows of a language's rule files use the same checks as `family: scope` (`params: { check: <id> }`, `message` with `{name}`, `{line}` and `{action}`), so a row carries its options, description and default level as data; JavaScript and TypeScript do ([../formatting/js.md](../formatting/js.md#scope-layer)):

| Check | Reads | Options |
|---|---|---|
| `undefined-variable` | a read or write no scope binds, minus the language's `globals` (`globals_if` by file flavor, `scope_names`), probes (`typeof x`) and tags (`<Tag />`); `top_unchecked` leaves the top scope alone | `globals` |
| `unused-variable`, `unused-parameter` | symbols never read, after ESLint's `no-unused-vars` and typescript-eslint (a read in its own `a = a + 1` or `a++` statement is none; exported, ambient, merged and property declarations are used; parameters after the last used one with `args: after-used`) | `caught_errors`, `ignore_pattern`, `ignore_rest_siblings`; `args`, `ignore_pattern` |
| `use-before-define` | a reference before its declaration, or evaluated while its variable is initialized | `functions`, `classes`, `variables` |
| `redeclared-variable` | a name declared again in a scope kind of `redeclared_in`; in typed files declaration merging is allowed | |
| `shadowed-variable` | a declaration that hides one of a scope around it (functions hoist, `var a = function a() {}` does not count) | `allow`, `kinds` |

The shared code names no node kind: it reads the grammar's role tags (`#function`, `#loop`, `#parameter-list`, `#assignment`, `#call`, `#block`, ...) and the tags a grammar adds for these rules (`#export`, `#export-list`, `#module`, `#module-body`, `#signature`, `#field`, `#sequence`, `#escapes`, `#default-value`, `#expression-statement`, `#markup`, `#decorator`, `#rest`, `#global-scope`), the fields of kinds (`init`, `body`, `left`, `right`, `iterable`, `callee`, `value`), the flags the table gives a declaration (`Flags`: `CONST`, `PARAMETER`, `IMPORT`, `EXPRESSION_NAME`, `UNCHECKED`, ...) and `ScopeLang`'s facts: `flavors` (`typed`, `definition` and others by file suffix), `globals`, `globals_if`, `scope_names`, `redeclared_in`, `parameter_properties` and `markup`.

Ruby (RuboCop Lint, [../formatting/ruby.md](../formatting/ruby.md#lint)) uses `unused-local-variable` for Lint/UselessAssignment and adds `unused-method-argument`, `unused-block-argument` (a parameter that nothing reads, in the scope kinds `ScopeLang::arguments` names, minus what its skip predicate allows) and `underscore-prefixed-variable-used` (an `_x` that is read).

Row options a language chooses (language neutral; each is a field of `Row`, `Declares`, `Sel`, `Flow` or `ScopeLang`):
- Names as tokens (`Row::tokens`, `ScopeLang::bare`): the own name tokens of a node declare (`Flow::Declare`), are not uses (`Flow::Skip`) or assign (`Flow::Assigns`: `=` writes, `+=` and `++` read and write, other operators pass the state on); selected by `Sel::First`, `Last`, `LastBefore(text)`, `All`, `BeforeOp`. With `bare`, a name token no row mentions is a use: Java has no node for a name, and reading too much only hides a finding.
- `Declares::name_last` and `skip_token` (an import declares the last name of its path or alias, none for `*`), `Declares::parent_flags` (a declaration whose flags depend on its parent), `Row::inert` (names under an import path are no uses).
- Scopes that do not see their surroundings (`Row::closed`: a PHP function), implicit declaration by a write (`Row::implicit`: Python names, PHP variables), `Flow::Capture` (a closure's `use (&$x)` declares in the closure and writes outside), `Sel::Kind` (children of one node kind).
- A scope the table cannot decide (`Row::dynamic` kinds such as PHP `include` and `$$x`, `ScopeLang::dynamic` names such as `extract`) is marked, and `undefined-variable` leaves it alone.
- `Place::Unseen` (a write declares only when no earlier name of the file was seen: Ruby `a = a`), `ScopeLang::labels` (token kinds of `key:` whose name is the shorthand `f(key:)` read), `ScopeLang::interpolated` (token kind whose `#{...}` words are reads: a heredoc body, which the tree holds as one token).
- `ScopeLang::docs` (comment prefix whose words use imports), `conventions` (imported functions called without being named), `globals` (names defined everywhere).

## Behavior summaries (`BehaviorSummaries`)
- `does_not_return`: `[DoesNotReturn]`, built-in list (`Environment.Exit`, `Environment.FailFast`), and `csharp: semantics: does_not_return:` (qualified names, every overload).
- `pure`: `[Pure]` and `semantics: pure:`; `is_pure` exists but no fix consumes it.

## Editor
The workspace folder is the worker's workspace. With `analysis: deep` the first pull reads the project's other files from disk (include/exclude, once per session or config reload) and the project phase runs over all of them; a pull returns the project-scope findings for that file.

## Compiler agreement gate
`crates/lang-csharp/tests/compiler_agreement.rs`: a 36-method fixture (jumps, loops, switches, try/finally, labels, lambdas, local functions, iterators, patterns, `using`, deconstruction, out variables); the flow rules must report exactly the compiler's CS0162, CS0168, CS0219 positions (`non_returning_calls: false`). On the three corpora (warning-clean code, so mostly a false-positive check): 0 differences in aspnetcore and Roslyn; one CS0219 in CoreLib (`PortableThreadPool.GateThread.cs`) the compiler reports and we do not (a struct local assigned `default` whose only use is a property read). `perf-bench semantic DIR [LIMIT]` runs it.
