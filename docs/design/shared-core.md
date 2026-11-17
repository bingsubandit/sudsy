# Design: keeping the cost of a language flat

Goal: a new language costs a grammar, a policy file and the code for what is unique to it. Today it costs a formatter and a rule set written from scratch, and the shared crates have stopped absorbing that cost. This note names the changes, their order and what each must prove. It is deleted when the last step lands; each step updates the docs of the area it changes.

## 1. Evidence (main at `ff508269`)
Hand-written Rust, generated files left out:

| | Languages | Shared crates (`core-*`, `provider-api`, `languages`) | `lang-*` crates | Shared share |
|---|---|---|---|---|
| Nov 3 | 3 | 16,237 | 9,167 | 63% |
| Nov 9 | 13 | 21,955 | 22,338 | 49% |
| Nov 16 | 26 | 24,631 | 62,593 | 28% |

- The sharing mechanism is the grammar file: a node kind carries role tags (`#member-chain`, `#binary-chain`, `#key-value`) and named fields, and shared shapes, shared rules and pattern rules read those. The languages behind an external parser do not use it. Node kinds with a role tag: C# 282 of 321, JS 2 of 191, Python 5 of 92, SQL 0 of 44, Kotlin 0 of 120, Java 45 of 149, PHP 30 of 169. Their formatters match on kind constants and find parts by position.
- Lists, operator chains, member chains and call arguments are written once per language: `lang-js/src/format/{call,chain,binary,assign,parens}.rs`, `lang-csharp/src/format/{chains,lists}.rs`, `lang-python/src/format/{op,list,paren}.rs`, `lang-java/src/format/expr.rs`. `core_format::shapes` is 1,096 lines; Python uses none of it.
- `core-semantics` and `core-flow` were used by C# only. JS reads the shared binder `core-semantics::scopes` through its own table (`semantic.rs`; S1); Python reads it through its own table too (`semantic.rs`; S2); `lang-js/src/rules/flow.rs` has its own reachability.
- Parser types are used outside the adapter in `lang-js` (5 files), `lang-python` (7), `lang-sql` (2), `lang-java` (2) and `lang-html` (2), so a parser bump or replacement is not a one-file change.
- The two generalization steps removed a net 431 lines of Rust. They moved provider glue and the adapter replay, which is not where a language's cost is.

Measured on the release CLI (4-core Xeon 2.1 GHz, x86-64 Linux, best of 5, check mode, cold process; ruff 0.16.9, Biome 2.6.8, oxlint 1.94.0, Prettier 3.8.1; commit `a4cbfbf`):

| Input | Task | Sudsy | Others |
|---|---|---|---|
| Django 5.2.8, 883 files, 5.7 MB | format / lint (F, E4, E7, E9) | 306 / 182 ms | ruff 80 / 51 ms |
| three.js and express, 8 copies, 20 MB | format | 1.70 s | Biome 1.19 s, Prettier 26 s |
| the same | lint, defaults | 0.83 s | oxlint 0.18 s, Biome 2.1 s |

Peak memory: 140 MB on Django (ruff 29 MB), 388 MB on the 20 MB of JS (oxlint 131 MB). A `.js` file nested between 2,000 and 4,000 levels overflows the stack and ends the whole run with no result for any file.

## 2. Target design
1. **The grammar is the contract.** Every node kind has a role from a fixed vocabulary, with named fields for the parts shared code reads, or is marked `#opaque`. Roles: call, member access, index, binary, unary, assignment, conditional, function, parameter list, block, declaration, type declaration, import, literal, scope, reference.
2. **Layout comes from a family kit; a language supplies policy.** A kit is a set of role-driven layouts in `core-format` (the brace-language kit: call arguments, member chains, operator chains, assignment, conditional, parentheses by precedence, statement blocks). A policy is a named value where reference tools differ (operator at the start or the end of a continuation line, how many calls break a chain, which arguments hug), declared in `lang.yaml` as an option with a default, so a preset can set it. C# already works this way (`fluid_assignments`, `chain_break_on_block`, `indent_nested_binary`). Markup (HTML and its dialects) and data languages (JSON, YAML, TOML, INI) are the other families and already share.
3. **One scope model and one flow builder, on our tree.** `core-semantics` gains an in-process model (scopes, bindings, references) built by a binder that reads roles, with each language's resolution rules as data (hoisting, function-wide locals, type and value namespaces) and a hook for what data cannot say. Unused, undefined, redeclared, shadowed and use-before-define are one rule family under each tool's ids; unreachable code runs on `core-flow` graphs built from statement roles.
4. **Parser types stay in the adapter.** A parser is borrowed when the language is large and moving and the upstream is well maintained; it is written by hand when the grammar is already forked, the format is small (a parser under about 1,000 lines), several dialects can share it, or the upstream has one maintainer or measures badly.
5. **Cost is measured and budgeted** per language, on what a language costs to become usable, not on its first step.

## 3. Steps
A step is one commit and one finish run. `After` names what must be on main first; steps with no entry can start now and run in parallel.

### Guards
| # | Step | Where | Done when | After |
|---|---|---|---|---|
| G2 | Parity and corpus runs as a scheduled CI job that fails on a ratchet | `.github/workflows`, `scripts/parity.sh` | every `differences.yaml` ratchet and every ignored conformance test runs nightly without a person | |
| G3 | Parser crates may be imported only by `adapter.rs`: a check with the files of section 1 as its starting list, which may only shrink | `xtask`, `clippy.toml` | the gate fails on a new import outside an adapter | |

### Roles
| # | Step | Where | Done when | After |
|---|---|---|---|---|
| R1 | The role vocabulary and the fields each role requires, as a table in `core-syntax` and in `docs/architecture/syntax-tree.md`; codegen refuses a node with neither a role nor `#opaque` once a grammar opts in | `core-syntax`, `xtask` | C# passes unchanged with its existing tags mapped to the roles | |
| R2 | Roles and field bodies for `js.grammar` and `java.grammar`; no formatter or rule change | `grammars/`, adapters only if a field needs a wrapped token | `ShapeChecker` passes on the whole corpus; parity counts unchanged | R1 |
| R3 | The same for Python, Kotlin, PHP, SQL | `grammars/` | as R2 | R1 |
| R4 | The vocabulary grows by the roles shared bug rules read: loop, switch and case, try and catch, return and throw, object or dict entry (key, value), and an operator kind on `#binary`/`#unary`/`#assignment` (comparison, equality, logical, arithmetic) as data of the grammar's operator tokens; the kinds R1-R3 marked `#opaque` for want of these roles get them | `core-syntax`, `grammars/` | every role-opted grammar maps them; `ShapeChecker` and parity unchanged | R2, R3 |

R1, R2 and R3 are in: the vocabulary is `core_syntax::roles::ROLES` (table in [../architecture/syntax-tree.md](../architecture/syntax-tree.md), with the counts per grammar); C#, JS, Java, Python, Kotlin, PHP and SQL opt in with `roles` and their `ShapeChecker` runs pass on the whole corpora. R2 decided the open questions there (roles only where the body declares every field as items: `if` is opaque, `new` is `#call`, a field may be several items). Left for L1 and S1:

R4 is in for C#, Python, Kotlin, PHP, SQL, Ruby (no `#try`) and HTML (roles and operator kinds: [../architecture/syntax-tree.md](../architecture/syntax-tree.md)); Open for K1: Java switch groups and switch expressions have no `#case` (labels and statements are sibling nodes); Python `ExprDict` has no entry node; Python `not in`/`is not` give the first operator token; PHP `if` keeps its `else` inside the `then` wrapper; SQL has no `#binary` yet.
- Roles are tags, so a role name is also a tag name in grammars that already used it (`#block`, `#declaration`, `#assignment`, `#literal` on C# tokens); role checks look at nodes only. A language whose old tag means something else under the same name has to rename the old tag.
- Java has no node for an identifier or a literal, so `#reference` and `#literal` have no Java kind; S1 and S4 decide whether the adapter wraps them. JS names that are keywords (`as const`, `yield`) are `IdentifierReference`s with a `Keyword` token.
- `Grammar::slot` reads a field at its fixed child position when every item before it is a single required child (field index built when the grammar loads), else matches the body; `slot_in` does the same on children already collected.
- Roles say nothing about precedence or operator text; L2's precedence table is a separate grammar declaration.

### Layout
| # | Step | Where | Done when | After |
|---|---|---|---|---|
| L1 | Pilot: member chains and call arguments move from `lang-js` into `core-format` as role-driven layouts with a policy struct; `lang-csharp` and `lang-java` are ported onto them | `core-format`, `lang-js`, `lang-csharp`, `lang-java` | all three parity ratchets within 2 points of their counts before the port (decision 1); bench within budget (the lines test is waived: the user accepts more shared lines than deleted ones, since the kit pays off with each further language) | R2 |
| L2 | The rest of the kit: operator chains, assignment, conditional, parentheses from a precedence table in the grammar, blocks and statement sequences | the same | as L1; each language's remaining formatter code is listed by construct in its doc | L1 passes |
| L3 | Kotlin and PHP formatters start on the kit: policy file first, Rust only for a named gap that cannot be a policy value | `lang-kotlin`, `lang-php` | formatter lines per grammar node recorded next to JS and C# | L2 |

Role-driven layouts read parts through `Grammar::slot` (the chain kit reads the children it already holds with `Grammar::slot_in`, an item behind single required items at its fixed position; `slot` itself walks the body as before, because a prebuilt field index or a fast path in `slot` cost the lint benches of tree-sitter languages 4 to 18 percent), which still costs more than a kind match; if a kit step's bench is over its format budget, look there first.

L1 result (accepted by the user; bench: java format +6.4% instructions and +8.4% allocations, component.html, vue and svelte format-query +2.4% to 4.3%, cs format allocations +2.7%, all else within budget; member chains moved, call arguments did not): `core-format::shapes::call_chain` (about 700 lines; the `member_chain_*` options are declared once in `core_config::shared`, a `lang.yaml` lists its defaults; styles `fluent` and `grouped`). Parity: JS 1110/2287 and C# 5489 of 7615 unchanged, second-format and refused unchanged; Java 4403 of 5337 (4504 before), the ratchet lowered by 101 files (1.9 points). Hand-written formatter lines: js 8567 to 8329, csharp 3671 to 3463, java 1694 to 1591 (-549 together) against +941 in the shared crates, so the shared share rose (27.3% to 28.2%) but the lines test of this row is not met: the kit is written once and a fourth language adds no chain code, yet three languages do not pay for it. What stayed language code: JS's `Hooks` (parentheses, comments, argument lists, `[expr]`, the simple-argument test) and Java's argument hook. Call arguments did not move: the three languages share only the last step of the layout (a conditional group of a flat, a hugged and an all-broken-out candidate) and differ in everything before it (JS hugs a last or first function, object or array with Prettier's tests, test calls, composition and React hooks; C# hugs a last block lambda with `lambda_argument_hug` and `closing_paren_line`; Java hugs only a sole block lambda or an annotation array and has no conditional group), so a shared skeleton would save about 40 lines and the predicates that make up the rest name TypeScript, JSX and lambda kinds. Java needed six named values and the remaining 101 files are quirks that are not yet policy values (a `this` head, call counting for non-simple heads, a chain inside a cast).

L1 is a test of the design. If the three languages cannot hold their ratchets on policy data alone, stop after L1, record why in this note, and treat the per-language formatter cost as fixed: the response is then a limit on the number of languages, not more abstraction.

### Shared rules
| # | Step | Where | Done when | After |
|---|---|---|---|---|
| K1 | Pattern rules match on roles and role fields (`#binary` with `operator: equality`, `left`, `right`), so one rule file runs on every grammar with roles; a shared family of the common bug rules on it (self-compare, constant condition, duplicate keys and cases, empty block and catch, assignment in a condition, comparison with a literal that is always the same), each tool's ids as rows (ESLint, ruff, Checkstyle, ktlint, PHPCS) | `core-patterns`, `core-rules`, the rule files of `lang-*` | the per-language copies of those rules are deleted; M4 coverage holds or rises for every tool with a corpus; lint bench within budget | R4, M4 |

### Scopes and flow
| # | Step | Where | Done when | After |
|---|---|---|---|---|
| S1 | The scope model and binder in `core-semantics`, with JS's binding table and resolution data; the shared rule family with ESLint's ids as rows; a parity mode that compares findings by rule and reported name | `core-semantics`, `core-rules`, `lang-js` | on the `eslint` and `typescript-eslint` parity corpora no problem the reference tool reports is missed (decision 2: compared by rule and reported name per file, not by position); `oxc_semantic` and the second parse are gone; `js.lint` within budget | R2, M4 |
| S2 (done) | Python on the same model (ruff ids as rows); the kept AST and `analysis/model.rs` go | `lang-python` | the same measure on the `ruff` parity corpus | S1, R3 |
| S3 | A flow-graph builder from statement roles; JS `no-unreachable` on `core-flow` | `core-flow`, `lang-js` | `rules/flow.rs` deleted, same findings (done: `core_flow::build`; `no-unreachable`, `getter-return` and `no-fallthrough` run on it) | R2 |
| S4 | Java, Kotlin and PHP get the scope rules from tables alone | `lang-*` | lines per language recorded | S1, R3 |

### Parsers
| # | Step | Where | Done when | After |
|---|---|---|---|---|
| P1 | The comparison note of `docs/design/php.md` is the standard for every new language, with two more rows: maintainers and funding, and whether tokens with trivia tile the source | `docs/architecture/adding-a-language.md` | written down | |
| P2 | One hand-written parser for the HTML family (HTML, Vue, Svelte, Angular as modes), replacing two forked grammars, one vendored grammar and `oxc-htmlish-parser` | `lang-html` | conformance and Prettier parity ratchets hold; parse instructions per byte recorded | G2 |
| P3 | Java: a replacement plan for `rezel-lang-java` | design notes | recommendation, the user decides | |
| P4 | Small formats (INI, gitignore, EditorConfig, TOML, GraphQL, Protobuf) get hand-written parsers when a fork would otherwise be edited or the language is being worked on anyway | `lang-*` | no step of its own | |
| P6 | Java: a hand-written parser, replacing `rezel-lang-java`; the tree keeps `java.grammar`'s node kinds and roles (R2), tokens tile the source | `lang-java` | 0 corpus parse errors (rezel: 2), no panic on text blocks, 10,000-link call chains parse, google-java-format parity and Checkstyle ratchets hold, rezel removed, speed and binary size recorded | R2 |

Kept as they are: oxc, ruff, Roslyn, mago, ruby-prism; tree-sitter for Bash, Markdown and PowerShell.

### Measures
| # | Step | Where | Done when | After |
|---|---|---|---|---|
| M1 | `scripts/cost.sh` reports, per language, hand-written lines by kind (formatter, rules, other) per grammar node, and the shared share of the workspace; the table in `adding-a-language.md` shows them for every language | `scripts`, docs | a CI check fails when the shared share falls or a language passes its budget (decision 3) without a line in the commit message | |
| M2 | Speed against ruff, Biome and oxlint as a script beside `speed-json-yaml.sh`, numbers in `numbers.md` | `tests/conformance`, docs | the table of section 1 can be re-run | |
| M3 | Parity catalogs list every upstream rule (ruff's lists 93 of its rules) as an inventory; their headline number is M4's coverage, not rules covered of N | `docs/parity` | counts regenerated | |
| M4 | Lint parity by function (decision 6): one mode of `scripts/parity_lint.py` for every tool runs the reference tool's default or recommended set on its real-world corpus and reports the share of its findings that the mapped Sudsy rule also reports in the same file, by rule and file (and reported name where the tool gives one), not by position; a ratchet on that share in `differences.yaml`. Fixture comparisons by `(rule, line)` stay only where the position is the check | `scripts/parity_lint.py`, `docs/development/parity.md` | every lint tool with a corpus reports its coverage share; S1 uses this mode | |

`adding-a-language.md` changes with L3 and S4: a language starts as a grammar with roles and a policy file, its formatter is the kit, and Rust is written per named gap.

## 4. Decisions
1. **A parity ratchet may fall by 2 points while its language is ported onto the kit.** Before the next language starts it is back at its count before the port, or at 90% of the real-world files.
2. **Rules replace the reference tools; they do not mirror them.** What counts is that a team that switches loses no check: every problem ESLint, ruff or another reference tool reports on the corpus is reported by the mapped Sudsy rule. Position, message and the choice among several equally valid reports may differ, and such a difference is a named `choice`, not a gap. The binder therefore has no per-tool switch.
3. **A new brace language has a budget of 3,000 hand-written lines to "usable"**; passing it needs a review.
4. **Release builds do not run the safety check.** It stays in debug builds, tests and the `corpus` profile.
5. **New languages go on while L1 runs**: parser comparison and step 1, with roles from the start. Formatter steps wait for L1's result.
6. **A language's lint is complete by function, not by mirroring output.** It is usable when, on real-world code, at least 95% of the findings of the reference tool's default or recommended rule set are reported by the mapped Sudsy rule in the same file (M4). Rules outside that set do not count toward usable; they stay `planned` entries.

## 5. What this gives up
- The last points of parity. A quirk of a reference tool that cannot be named as a style option stays a difference, or becomes per-language code, which is the cost this plan removes.
- A porting period in which JS, C#, Java and Python formatter work is rewriting, not new parity.
- oxc's scope analysis: the binder reimplements it. Java, Kotlin, PHP and Ruby have no such crate to borrow.
- Independence between languages: a kit change moves the output of every language on it, which is why G2 comes first.
- Some speed in role-driven code against kind-matched code; the tree build becomes the hot path of every rule, so its cost (six sevenths of the JS parse) is the next thing to cut.
