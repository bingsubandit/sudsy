# Roadmap: not built, or deferred

One line each; none is committed, except the small tasks below.

## Distance to the usability bar
Per language against the bar in [CLAUDE.md](../../CLAUDE.md) (format 85-90% identical to the reference tool on real-world code, `second-format` 0, lint 85% of the reference set's findings). Distance = shortfall of format below 85% plus shortfall of lint below 85%, a leg with no reference run or no real-world corpus counted as the full 85 (`?`, itself a gap). Numbers: [../languages.md](../languages.md) (format, tool versions), [parity/README.md](parity/README.md#lint-coverage-by-function) (lint, preset, missing rules). Format counts re-measured with `scripts/parity.sh format <tool>`; second-format is 0 wherever a run reports it.

| # | Language | Format identical (tool) | Lint coverage (tool, preset) | Biggest causes |
|---|---|---|---|---|
| 1 | C / C++ | 16.2% (statements and blocks landed, vs clang-format 20.1.8: 1,075 of 6,639 files, LLVM 230/420, folly 395/2,299, Linux 352/2,872, range-v3 70/593, fmt 24/70, SQLite 4/325, presets 0/10 each) | none (no clang-tidy catalog) | no layout yet: formatter on shared shapes in progress ([design/cpp-formatter.md](design/cpp-formatter.md)); no lint rules |
| 2 | JSON | ? (fixtures only, no real-world corpus) | ? (eslint-plugin-jsonc; corpus is JSONTestSuite) | no real-world corpus; no coverage run |
| 3 | XML | ? (CSharpier XML, 372 project files, count not recorded) | none | no corpus count; no reference linter |
| 4 | GitHub Actions | none (YAML formatter, no reference) | 5.4% (actionlint, `github-actions/actionlint`) | `syntax-check` 235, `expression/types` 155, `events` 115 findings |
| 5 | Dockerfile | none (no reference) | 27.6% (hadolint, `dockerfile/hadolint`) | `DL4006` 153, `DL3003` 64, `DL3008` 54 |
| 6 | C# | 72.1% of 7,615 (CSharpier 1.3.0; not re-run, corpus absent on the host) | ? (StyleCop; corpus does not compile) | lists differing by a comma, `#if` comments (252 files), wrapped arguments and chains (299) |
| 7 | Makefile | none (no reference) | 73.5% (checkmake, `makefile/checkmake`) | `uniquetargets` 88, `phonydeclared` 59 |
| 8 | YAML | ? (no Prettier corpus count) | 73.8% (yamllint, `yaml/yamllint`) | no real-world format corpus; `comments` 325, `indentation` 160, `syntax` 136 |
| 9 | Kotlin | 69.4% of 5,776 (ktlint 1.8.0) | 13.2% (ktlint, `kotlin/ktlint`) | planned layout rules `function-signature` 30,800, `argument-list-wrapping` 24,113; rewrites (`: Unit`, `;`, modifier order) |
| 10 | SQL | 78.2% of 867 (sqlfluff 4.5.0) | 4.4% (sqlfluff, `sql/sqlfluff`) | planned `CP02` 5,770, `CP01` 3,992, `LT05`; 189 parse gaps and 189 layout gaps |
| 11 | PHP | 88.5% of 10,382 (PHP-CS-Fixer 3.95.31) | 0.0% (PHPStan, `php/phpstan`) | type analysis (`class.notFound` 7,407, `variable.undefined` 591); inline-text files (~790) |
| 12 | Ruby | none (no reference) | 96.2% (RuboCop Lint, `ruby/rubocop`) | no formatter run |
| 13 | GraphQL | 97.7% of 132 (Prettier) | ? (graphql-eslint, no coverage run) | no coverage run |
| 14 | Terraform (HCL) | 98.1% of 318 (terraform fmt) | 19.6% (tflint, `hcl/tflint-recommended`) | module-level rules: `terraform_required_providers` 37, `unused_declarations` 31, `typed_variables` 27 |
| 15 | Java | 82.5% of 5,337 (google-java-format 2.0.0) | 25.5% (Checkstyle, `java/google`) | `MissingJavadocMethod` 18,283, `MethodName` 7,887, `JavadocParagraph` 5,301; 934 files with layout gaps (line comments, chains in ternaries) |
| 16 | PowerShell | 92.9% of 211 (Invoke-Formatter; not re-run, no `pwsh` on the host) | 34.4% (PSScriptAnalyzer) | `PSUseShouldProcessForStateChangingFunctions` 140, `PSUseDeclaredVarsMoreThanAssignments` 130 |
| 17 | Shell | 86.6% of 82 (shfmt 3.14.1) | 86.6% (ShellCheck, `shell/shellcheck`) | `SC1091` 12 and `SC1090` 2 (source following), `SC2235` 2; backticks, `case` in `if`, arithmetic spacing |
| 18 | HTML | 92.7% of 233 (Prettier) | 63.7% (html-validate; htmlhint 5.0%, html-eslint 0.3%) | `wcag/h63` 48, `parser-error` 21; html-eslint `indent` 16,182 planned; embedded code 56 files |
| 19 | Markdown | 85.9% of 64 (Prettier; small corpus) | 82.7% (markdownlint, `markdown/markdownlint`) | `MD010` 309 (tabs); corpus of 64 files |
| 20 | Vue | 83.1% of 941 (Prettier) | 87.2% (eslint-plugin-vue) | `vue-js-layout` 183 files; `vue/attributes-order` 81 |
| 21 | Svelte | 89.2% of 1,055 (Prettier + plugin) | 100.0% (eslint-plugin-svelte) | `svelte-attributes` 139 files; second-format not measured |
| 22 | Protobuf | 99.1% of 323 (buf format) | 85.9% (buf, `protobuf/buf-standard`) | at the bar; `RPC_REQUEST_RESPONSE_UNIQUE` 117 (cross-file) |
| 23 | CSS / SCSS / Less | 92.7% of 533 (Prettier, `--no-config`) | 94.1% (stylelint, `css/stylelint-standard`) | at the bar; `media-feature-range-notation` 415, `value-keyword-case` 127, `no-descending-specificity` 74; SCSS layout and comments |
| 24 | TOML | 95.3% of 213 (taplo; fixtures) | no reference linter | at the bar; second-format not measured |
| 25 | JavaScript / TypeScript | 93.2% of 2,456 (Prettier) | 100% (ESLint, typescript-eslint) | at the bar; Prettier options of the projects (168 files) |
| 26 | Python | 92.7% of 1,085 (ruff format 0.17.4) | 95.6% (ruff, `python/ruff`) | at the bar; `RUF100` 83 |

Not ranked: INI, EditorConfig and `.gitignore` have no reference tool. Measured by a re-run on this host: JS, CSS, HTML, Vue, Svelte, Java, Python, Shell, Protobuf, Terraform, TOML, SQL, Kotlin, PHP; recorded and not re-run: C#, PowerShell, Markdown, GraphQL, XML and all lint shares.

## Small tasks
- perf-bench builds the base tree with the candidate's tools/perf-bench, so a `FileContext`/`parse_regions` API change fails the base build; build the base with its own perf-bench.
- `Flow::seq` wraps only flows with a hung part in the `ALIGN_FROM_LINE` align (php.format +1.43% under `source_hang`); prove it with a cpp parity run.
- Read shared rule specs once per process, not per embedded language.
- Pattern rules capture a string label (`variable "$NAME"`), then name the variable in the `typed-variables` message (`crates/lang-hcl/rules/declarations.yaml`).
- Human docs (docs/*.md, README.md) carry numbers copied from docs/ai; refresh them when a parity or performance number changes.
- Format-string family: dispatch once per node instead of 16 `MEMO.names` calls (py.lint +3.6%).
- The 13 pattern rules of `lang-shell/rules/words.yaml` cost +194% shell lint (6.5 M to 19.2 M instructions) when on: measure the matching cost of pattern rules (regexes on the text of every `Command`, `deep` token scans of the whole program) before any of them goes default-on.
- Honour `# shellcheck disable=SC....` comments as a shared suppress-alias that a preset maps to its rules, so the shellcheck preset stays quiet where the file silences shellcheck.
- Profile the `core_format::spacing` row lookup (kt.format +6.4%, java.format +4% at F2) once a profiler runs on the agent host.
- Scope family: JS `unused-import` is still part of `unused-variable` (split it as Java, Kotlin and PHP have it); Python's `F401`/`F841` rows move on the family in L3.
- Flow family modeling gaps (`unreachable-code`, `no-fallthrough`; [rules/semantics.md](rules/semantics.md#flow-family-core_rulesflow_rules)): JS static blocks and TS module blocks are not units of their own; PHP file level and `if`/`else` bodies (opaque wrappers); Java arrow `switch` arms and `for` heads; Ruby `begin`/`rescue`; Python `with` and `try`/`else`.
- Layout rules that are `planned` in the catalogs while a formatter could own them (their findings are most of the gap in [parity/README.md](parity/README.md#lint-coverage-by-function)): html-eslint `indent`, `element-newline`, `attrs-newline`; ktlint `function-signature`, `argument-list-wrapping`, `multiline-expression-wrapping`, `class-signature`, the trailing-comma and wrapping rules; sqlfluff `CP01`-`CP05` (capitalisation), `LT05`. Mark them `formatter` (the coverage run then leaves them out) or write the rule.
- Python lint cost: `py.lint` is about +60 percent over the typed-AST pass it replaced (accepted by the lead; single-walk binder replaces the typed-AST pass). Profile of a 5-file pass (4.0 ms, release): binder 1.3 ms (0.4 of it the bare rowan preorder, name references with resolve 0.55, subscript and argument rows 0.2), scope findings 0.8, engine walk 0.3, the other ~50 rules 1.4 (about 0.03 each). Next: drive the binder from the engine's own preorder, and per-node memo of the `check` rules that share an analysis.
- Python lint: cut the allocations of the ruff-family `check` rules (one shared walk or memo for isort/branch/literal/types, no per-node String/Vec); py.lint +67% instr / +95% allocs at 95.6% coverage.
- Java argument lists are still `paren_list_in` (`decl.rs`): `shapes::delimited_list` items are nodes, and Java expressions are bare tokens; with token items the list, the format-string group and the annotation arrays go onto `shapes::call_arguments`. Kotlin and PHP argument lists never hug (ktlint, PER-CS), they stay delimited lists.
- Java mechanisms still in `lang-java/src/format/` that belong in core-format on kind sets: `paren_list_in` (tabular-argument branch, blank before a commented item, no hug in a chain head, lone expression-lambda `fits_below`) onto `shapes::call_arguments`; `throws` (hanging list) onto `shapes::heritage`/`list`; the array initializer (tabular branch, fill when every item is under 10 characters as a `fill_max_item_width` option); the multi-element `switch_rule` body onto a shared arrow-rule shape.
- Java call arguments (hug rules) wait for the object, array and template roles of R4; the `member_chain_*` option declarations repeat in three `lang.yaml` files and want one shared declaration.
- Ruby lint: generalization step (the `check` family of `lang-ruby` and the Java/Kotlin/PHP/Ruby scope tables); close the named gaps of `docs/ai/parity/rubocop.yaml` (UselessAssignment overwritten-before-read cases, 168 findings; the `planned` cops), read `# rubocop:disable` comments, and report `a && b = c` at the assignment line.
- Java: generalization step ([architecture/adding-a-language.md](architecture/adding-a-language.md#after-a-language-lands-the-generalization-step)).
- Java formatter gaps (named in `tests/parity/google-java-format/differences.yaml`, 91.6% identical): drop `()` from `@A()` (the safety check compares node kinds, so the token pair needs `#optional`); chains with a field-access head; a text block's closing quotes; wrapping trailing comments; AOSP's import groups.
- PHP formatter gaps (named in `docs/ai/formatting/php.md`, 9,189 of 10,382 identical): format the PHP islands of files with inline text, one `use` per trait and the blank after `<?php` (W1 rewrite rows), parameter attributes on their own line, `case x: stmt;` and `// no break` placement, comments after an operator at a line end, rewrites for `else if`, braces, modifier order and import order; PHP: generalization step ([architecture/adding-a-language.md](architecture/adding-a-language.md#after-a-language-lands-the-generalization-step)).
- Flow::between calls the op_first, joined and at_least_one closures up to three times per token pair; cache op_first(a) once per pair (Kotlin format is +1.02% over the shared Flow)
- Kotlin: generalization step ([architecture/adding-a-language.md](architecture/adding-a-language.md#after-a-language-lands-the-generalization-step)); the `intellij_idea` and `android_studio` code styles as options; `.kts` and Gradle Kotlin DSL checks.
- Kotlin formatter (signatures, parameter and argument lists, trailing commas, import order, expression bodies and braces are in): binary chains on the kit, the `: Unit`, `;` and comment-spacing rewrites (a shared rewrite layer first), blank lines in lambdas, width of values moved below `=`, `string-template-indent`, measured by `scripts/parity.sh format ktlint` (the `real-world` ratchet); Kotlin lint: the `planned` entries of `docs/ai/parity/ktlint.yaml` (`no-unused-imports`, `string-template`, naming rules).
- Java lint: the `planned` entries of `docs/ai/parity/checkstyle.yaml` (`FallThrough`, `InnerAssignment`, `MultipleVariableDeclarations`, Javadoc checks, `RedundantModifier`, metrics); a differential runner against Checkstyle needs a JVM, with the shared JRE helper `scripts/lib/jre.sh`.
- Angular: files other than `*.component.html` (`app.html`) are plain HTML, and `overrides` cannot map a glob to a language; a `languages`-style file association (glob to language and dialect) would let a project choose.
Committed work too small for a step of its own: what to do and where, one line each. Once the list holds about a step's worth (5 or more related items, or a day of work), the lead hands it to one agent as a single step; the commit that does an item deletes its line. Bugs go to [bugs.md](bugs.md) instead.
- Grammar tables as static codegen data (labels, names, `by_name` keys still `String`; ~1650 allocations per JS load).
- Pattern rules (`core-patterns`): each rule compiles its regexes at load, about 430 allocations per rule; 35 Vue rules as pattern rules cost vue lint +110% instructions, so they became the Rust `template` family. Share or lazily build the regexes so pattern rules stay the cheap default (measure with the html/vue lint bench).
- PowerShell: `scripts/parity.sh format invoke-formatter` needs `pwsh`, which the agent host lacks: measure its second-format count and add the ratchet to `tests/parity/invoke-formatter/differences.yaml`; bring it to 0.
- HTML dialect rules: the remaining planned entries of `docs/ai/parity/eslint-plugin-vue.yaml` and `eslint-plugin-svelte.yaml` (`no-useless-template-attributes`, `valid-template-root`, `prefer-class-directive`, the `no-restricted-*` ones); a plain pattern rule costs a regex per rule in the lint bench, so new ones go into the `template` family. The Vue and Svelte coverage runs (all findings of `flat/recommended`: Vue 99.6%, Svelte 100%) name what is left: `no-template-shadow` (the template's `v-for` and slot variables against the script's names), `multi-word-component-names`, `return-in-computed-property`; the other `<script>` rules are `planned` in the catalog and go into the `script` family (`lang-html/src/script`) on the scope model of `lang_js::bind_embedded`. Svelte `<script>` regions are not wired yet. Fixture runners (`scripts/parity.sh`) for html-eslint and HTMLHint, as for html-validate (their coverage runs exist).

## Languages and analysis
- JavaScript / TypeScript: Flow is unsupported; type-aware rules wait for the TypeScript worker.
- SQL: dialects beyond ansi, postgres, mysql and tsql (not written yet); templated (Jinja, dbt) files; migration (`sudsy init --from sqlfluff` and `--from sqruff`: `.sqlfluff`, `setup.cfg`, `tox.ini`, `pyproject.toml`, `.sqruff`, `exclude_rules`, per-rule sections; `dialect` outside the four is reported); `sql` fences in Markdown and SQL inside other languages' strings; comment placement in the layout; the sqlfluff `layout.type` and `indentation` settings beyond the options; the planned sqlfluff rules by usage ([parity/sqlfluff.yaml](parity/sqlfluff.yaml): reference RF rules first); a real-world dbt-free SQL project in the parity corpus.
- CSS / SCSS / Less: the stylelint-scss and stylelint-less rules ([parity/stylelint-scss.yaml](parity/stylelint-scss.yaml), [parity/stylelint-less.yaml](parity/stylelint-less.yaml)) and the core rules not built ([formatting/css.md](formatting/css.md)); the Prettier layout gaps named in `tests/parity/prettier-css/differences.yaml` (SCSS maps and comments, Less mixin calls, comments in selectors, front matter); real-world parity runs under each project's own Prettier config; `css` template tags in JS; the Sass indented syntax (`.sass`, Prettier does not format it).
- Python: migration (`sudsy init --from ruff` and `--from black`: `pyproject.toml` `[tool.ruff]` and `[tool.black]`, `ruff.toml`, `per-file-ignores` and `extend-exclude` as `overrides` and `exclude`); docstring code formatting (off in ruff's defaults) and the layout differences of `tests/parity/ruff/differences.yaml`; more ruff families by usage (the rest of `B`, `UP`, `SIM`, `C4`, `PIE`; `I`, `N`, `PL`, `RET`, `D`); notebooks (ruff's `ruff_notebook`); type-aware rules through Pyrefly (below).
- INI: `.properties` (`:` and blank separators, `\` continuations, `!` comments); a `.gitconfig` dialect that allows repeated multi-value keys.
- C / C++: the clang-format-compatible formatter (LLVM default and the Google, Chromium, Mozilla, WebKit, GNU and Microsoft presets as options; `ForEachMacros`, `StatementMacros`, `AttributeMacros`, `TypenameMacros`, `NamespaceMacros`, `MacroBlockBegin/End`; parity against clang-format 20.1.8 with each corpus project's own `.clang-format`); syntax-level lint rules on the token and line structure (clang-tidy needs a libclang worker); the design note [design/cpp.md](design/cpp.md) is deleted when the formatter is usable.
- TOML: TOML 1.1 syntax (the tree-sitter grammar targets 1.0; needs an upstream fix or a fork).
- XML: the inside of a DOCTYPE (the grammar cannot parse every DTD declaration) and DTD-dependent well-formedness (entities, attribute defaults); reflowing text (the formatter keeps mixed content as written); a `xmlWhitespaceSensitivity: ignore` mode; sorting `PackageReference` items.
- Gitignore syntax: `.dockerignore`-specific rules (root-only `*.log`).
- Dockerfile: rules (hadolint parity); shell code inside `RUN` as a Shell language injected through the composite adapter (the formatter keeps it as written); (low priority) vendor tree-sitter-containerfile's `scanner.c` with a fix for consecutive `[`-led continuation lines, which make 24 corpus files unparseable ([formatting/dockerfile.md](formatting/dockerfile.md)).
- Shell: rules (shellcheck subset); respacing the operators inside `(( ))` and `$(( ))` like shfmt ([formatting/shell.md](formatting/shell.md)).
- PowerShell: rules (PSScriptAnalyzer subset) ([formatting/powershell.md](formatting/powershell.md)).
- Makefile: shell code in `$(shell ...)` and `define` bodies; rules beyond checkmake's five.
- GitHub Actions: the actionlint checks not built ([parity/actionlint.yaml](parity/actionlint.yaml): types of expressions, events, globs, matrix, permission scopes, action metadata); a matcher for `.github/workflows` as a path, not a folder name ([formatting/github-actions.md](formatting/github-actions.md)).
- Protobuf: the cross-file buf lint rules ([parity/buf.yaml](parity/buf.yaml)); the remaining `buf format` differences ([formatting/protobuf.md](formatting/protobuf.md)).
- GraphQL: the graphql-eslint rules that need no schema and are not built (`docs/ai/parity/graphql-eslint.yaml`), schema-aware rules.
- HCL: the tflint rules that need the other files of the module or the providers, `terraform_deprecated_lookup` and the module source rules ([parity/tflint.yaml](parity/tflint.yaml)); the rewrites `terraform fmt` does besides layout (quoted block labels, quoted `variable` types), which would be rules ([formatting/hcl.md](formatting/hcl.md)); `sudsy-ignore` in HCL.
- Many more languages, each with a formatter and rules matching its de-facto tools, native rules only, a worker process per language where its valuable rules need types (like the C# worker). Ordered by coverage and cost, languages whose toolchain ships a formatter or linter last: INI / `.properties` / `.gitconfig`, ignore files, GraphQL; JavaScript / TypeScript / JSX / TSX (Prettier, ESLint, typescript-eslint), CSS / SCSS / Less (stylelint), HTML, Vue / Svelte; Python (ruff, black), SQL (sqlfluff), Java (google-java-format, Checkstyle), Kotlin (ktlint: parser, whitespace formatter and first rules are in), C / C++ (clang-format; formatter first, type-aware clang-tidy rules with the libclang worker), Razor, PHP (PER-CS / PHP-CS-Fixer; parser comparison in [design/php.md](design/php.md)), Ruby (RuboCop), Lua, R, Scala, Haskell; then the languages with official tools: Swift, Elixir, Dart, Go, Rust, Zig ([architecture/adding-a-language.md](architecture/adding-a-language.md)).
- Languages whose toolchain ships the formatter (Go, Rust, Dart, Zig, Swift, Elixir): Sudsy lints them with its own rules and does not format them at first; it never wraps the official formatter (its options, presets, rule fixes, embedded code and editor path would bypass Sudsy). Before the first of them starts, measure where an own formatter on the Doc printer could beat the official one (speed, editor latency, options it lacks, such as Swift's two competing formatters) and build one only where it does.
- Language packs, remaining: signing the packs (they are hash-checked against the embedded manifest, not signed).
- Plugin (Wasm) overhead: encode the tree once per file for all plugin rules, run a plugin's rules in one call, send only the node kinds it asks for.
- Type-aware rules per language use the language's reference checker, never our own, through a worker (the C# worker's protocol): Python with Pyrefly (preferred) or ty in-process if either can be used as a Rust library (spike first; pyright is too slow), TypeScript on the native compiler's API (7.x beta API now, the stable 7.1 API when it lands), Java (Eclipse JDT) and Kotlin (Analysis API) in one JVM host, then C / C++ (libclang). After the syntax-level sweep of the languages above; none for Ruby, Go, Rust, Swift for now.
- Replacing a tree-sitter grammar with a hand-written parser where speed or error messages matter.
- Native C# parser (Roslyn stays the only one; the conformance suite and addressing by `(file, version, span, kind)` keep the door open).
- `MSBuildWorkspace` / project- and NuGet-aware binding with syntax-only fallback (replaces `Workspace.cs` without a protocol change).
- Point queries (`symbol_of`, `type_of(span)`) and the pattern predicate `type_matches` that needs them.
- Taint and resource-state analyses on `core-flow`; call-graph rules; `must_use_result`, ownership and taint summaries; a consumer for `pure`.
- Embedded languages in Razor and in C# raw strings; Markdown fences of C# (the worker would start for a snippet; see [architecture/adding-a-language.md](architecture/adding-a-language.md#embedded-languages)).
- `preprocessor_symbols` come from config only, language version is not configurable.
- Rules: `var-preference`, `prefer-collection-expression`; a rename fix for `async-suffix`; rules using reaching definitions / definite assignment.
- Semantic requests in parallel (one workspace per core).
- Migration aid mapping StyleCop / .NET analyzer ids and `.editorconfig` severities to our rules. Hosting Roslyn analyzers is a non-goal.

## Formatting
- Range (selection) formatting; the formatter can already format a subtree in context.
- Other CSharpier differences listed in [formatting/csharp.md](formatting/csharp.md).
- Rule-produced layout directives for `#if` branches formatted from other parses (the layout rules run on the file's own parse only); solving `#if` conditions to find symbol sets (`derive_symbol_sets` tries subsets of the condition symbols, at most 16 sets).
- A rule source that attaches layout directives (pattern `layout:` key, plugin directive output); plugins returning layout documents is deliberately excluded.
- YAML: re-wrapping long scalars and comments; comments between split properties.
- Performance levers: green-tree cursor for the formatter instead of rowan red nodes; sharing anchored YAML nodes (`Rc`) for alias-heavy files; first-token index for pattern rules.

## Extensibility
- Plugin SDKs for languages other than Rust (the WIT is documented); freezing the plugin API at v1.
- Host import for parsing snippets so plugins can compile patterns; sharing one decoded tree across the rule calls of a plugin; options schema for plugin rules.
- Captures kept through `inside`/`has`.
- A separate plugin-host binary to shrink the 28.3 MB binary (LTO measured: 7-12% smaller for 2-3x build time, see performance/numbers.md).

## Tooling and editor
- Pull-diagnostic result ids; editor watching of closed files for deep diagnostics; pre-starting the second worker so formatting during the first build is instant.
- SARIF `fixes`; an output-file option.
- A result cache across CLI runs (as `ruff`, `eslint --cache`): per file, keyed by its bytes, the Sudsy version, its resolved config, plugin and pattern file hashes and pack versions; semantic results are additionally keyed by a hash of every file of that language in the project, so a change that touches no file of the language (a docs-only or YAML-only commit in a mixed repository) keeps them. Per-result file dependencies (re-checking only the files that use what changed) are the finer step, the same fix the LSP's stale cross-file diagnostics need. Built when a measured CLI run on a large repository (pre-commit, CI) is too slow.
- Editor extensions other than VS Code (other editors use `sudsy lsp`).
- Self-contained C# worker for machines without .NET.
- Repeatable benchmarks for the paths listed in [performance/numbers.md](performance/numbers.md).
- Publish the documentation site ([development/site.md](development/site.md)): the Pages deploy runs from main; switch `DOCS_BASE_URL` on and add `codeDescription` links to diagnostics.
- Release: first Marketplace publish ([development/ci-release.md](development/ci-release.md)).
