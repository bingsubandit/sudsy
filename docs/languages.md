# Languages

How each language is measured, with corpora, tool versions, presets, options and differences. The sections follow the order of the table in the [README](../README.md#language-support): the ready languages first, then the others, each by how widely the language is used.

## How to read the numbers

The status and the numbers of every language are in the generated table "Language support" of the [README](../README.md#language-support) and, with the counts behind them, in the first lines of each section below. No number is typed on this page: they come from the parity runs (`tests/parity/<tool>/differences.yaml`). The page holds what the numbers leave out: corpora, tool versions, presets, options and the main differences.

- **Format:** share of corpus files where `sudsy format` gives the same bytes as the reference formatter. The corpus is real-world code unless the section says otherwise.
- **C / C++ format** does not compare line wrapping (where a statement, a list or an enum breaks, the indent of continuation lines); see the C / C++ section.
- **Lint:** share of the reference linter's findings on the corpus that Sudsy also reports with the named preset, on the same line. This is recall. False positives are not counted.
- Every number is measured against the reference tool's default rule set (formatters: default settings), not your project's config. A row measured with another set says so in its section. See [keep your current style](configuration.md#keep-your-current-style).
- With no config, lint reports likely bugs only. Lint numbers need the preset. See [configuration](configuration.md#presets).
- A lint column with no reference linter shows the number of rules Sudsy has: every rule available to the language, its own and the shared ones, counted from the rule registry. It does not decide `Ready`.
- A format column with no reference formatter shows `stable`: the share of the language's corpus files that parse, format with the meaning check passing, format to the same text a second time and are laid out by the formatter's rules, not copied through. It decides `Ready` like a reference tool's number (at least 85%). For a language added after the six oldest such legs (INI, EditorConfig, .gitignore, Dockerfile, Makefile, GitHub Actions) the bars are higher: `stable` 100% and an agreement of 80% (the share of the format decisions that real code already follows), and for a lint leg with no reference linter a checklist of 85% (the share of a written list of known problems that a rule reports) and a precision of 90% (the share of reported findings a reader judges true).
- Both numbers leave out only reviewed intentional differences (`tool-wrong`, `invalid-input`, `wontfix`). The lists are in `tests/parity/<tool>/differences.yaml` and [ai/parity/README.md](ai/parity/README.md).
- Reproduce a number: `scripts/parity.sh <tool>` (format), `scripts/parity.sh coverage <tool>` (lint), `scripts/parity.sh stability <group>` (stable).
- Every number comes from a pinned tool version. Versions are in the sections below.
- Language names in `sudsy.yaml` sections are the ones the editor schema lists: `js` (JavaScript and TypeScript), `python`, `hcl`, `shell`, and so on.

## Options every formatter shares

`line_width`, `indent_size`, `indent_style` (`space` or `tab`), `final_newline`, `end_of_line` (`auto`, `lf`, `crlf`), `bracket_spacing` and `array_bracket_spacing`. A language ignores a shared option it does not declare. Set them once at the top level, or per language. See [configuration](configuration.md#format-options). Each section lists only the options that go beyond these.

## HTML, Vue, Svelte, Angular

<!-- generated:measured-html-vue-svelte-angular -->
- **Measured (HTML):** Ready.
  - Format, Prettier: 95% of the files identical (506 of 532 files).
  - Lint, html-validate: 92% of its findings also reported (2,246 of 2,428 findings).
  - Lint, html-eslint: 88% of its findings also reported (281 of 319 findings).
  - Lint, htmlhint: 91% of its findings also reported (1,244 of 1,358 findings).
- **Measured (Vue / Svelte):** Ready.
  - Format, Prettier: 91% of the files identical (1,817 of 1,996 files).
  - Lint, eslint-plugin-vue: 88% of its findings also reported (1,392 of 1,567 findings).
  - Lint, eslint-plugin-svelte: 100% of its findings also reported (67 of 67 findings).
- **Measured (Angular templates):** Ready.
  - Format, Prettier: 99% of the files identical (1,134 of 1,139 files).
  - Lint, angular-eslint: 99% of its findings also reported (487 of 488 findings).
<!-- /generated -->

- One parser serves all four. Config sections: `html`, `vue`, `svelte`, `angular`.
- **Formatter:** Prettier 3.9.10 (Svelte: with prettier-plugin-svelte 3.5.2).

- **Corpora:** real-world files (the HTML group adds the demo pages of mdn's dom, webaudio, web-components, pwa, js, html and webgl examples, Web-Dev-For-Beginners and html5-boilerplate) and Prettier's own tests for HTML, Vue and Svelte; the prettier-plugin-svelte samples; Angular components and Prettier's Angular tests (`--parser angular`).
- **Linters:** html-validate, htmlhint, html-eslint, eslint-plugin-vue, eslint-plugin-svelte. Angular: a few angular-eslint rules, no coverage run.
- **Presets:** `html/html-validate-recommended`, `html/html-validate-standard`, `html/htmlhint`, `html/html-eslint-recommended`, `html/eslint-plugin-vue-recommended`, `html/eslint-plugin-svelte-recommended`.
- **Options:** `whitespace_sensitivity`, `bracket_same_line`, `single_attribute_per_line`, `embedded_language_formatting`. Vue and Svelte add `indent_script_and_style`. Default `line_width` is 80.
- **Differences:** Vue and Svelte differ most. The named gaps are in the source checkout.
- **Reproduce (needs a source checkout):** `scripts/parity.sh format prettier-html`, `scripts/parity.sh format prettier-angular`, `scripts/parity.sh coverage html-validate`.

## JavaScript / TypeScript

<!-- generated:measured-javascript-typescript -->
- **Measured:** Ready.
  - Format, Prettier: 95% of the files identical (2,357 of 2,456 files).
  - Lint, ESLint: 100% of its findings also reported (4,413 of 4,413 findings).
  - Lint, typescript-eslint: 100% of its findings also reported (3,644 of 3,644 findings).
<!-- /generated -->

- Config section: `js`.
- **Formatter:** Prettier 3.9.10 (`--no-config`).
  - Corpus: real-world projects and Prettier's own tests.
- **Linter:** ESLint recommended rules and typescript-eslint recommended. Core rules only. No react, import or a11y plugins. No type-aware rules.
- **Presets:** `js/eslint-recommended`, `js/typescript-eslint-recommended`, `js/biome`, `js/biome-recommended`, `js/oxlint`.
- **Options:** `semi`, `quote_style`, `quote_props`, `trailing_commas`, `arrow_parens`, `bracket_spacing`, `bracket_same_line`, `jsx_single_quote`, `member_chain_style`. Default `line_width` is 80.
- **Differences:**
  - A line comment in a class or interface header moves into the body.
  - JSX as the object of a member access keeps its parentheses.
  - Syntax the parser rejects gives a syntax error: pipeline operator, do expressions, Flow, bind `::`.
- **Reproduce (needs a source checkout):** `scripts/parity.sh format prettier-js`, `scripts/parity.sh coverage eslint`, `scripts/parity.sh coverage typescript-eslint`.

## CSS / SCSS / Less

<!-- generated:measured-css-scss-less -->
- **Measured:** Ready.
  - Format, Prettier: 92% of the files identical (494 of 533 files).
  - Lint, stylelint: 94% of its findings also reported (12,023 of 12,686 findings).
  - Lint, stylelint-scss: 89% of its findings also reported (1,818 of 2,022 findings).
  - Lint, stylelint-less: 85% of its findings also reported (8,494 of 9,947 findings).
<!-- /generated -->

- Config section: `css` (also `scss`, `less`).
- **Formatter:** Prettier 3.9.10 (`--no-config`). Corpus: real-world projects and Prettier's own CSS, SCSS and Less tests; the conformance corpus (Bootstrap, Bulma and others).
- **Linter:** stylelint standard. Also stylelint-config-standard-scss (the `scss/` plugin rules) and stylelint-less, each with its own number.
- **Presets:** `css/stylelint-recommended`, `css/stylelint-standard`.
- **Options:** `single_quote`, `max_blank_lines`. Default `line_width` is 80.
- **Differences:**
  - Some SCSS maps and `@include` with comments stay as written.
  - Less mixin calls with `{ }` arguments stay as written.
  - A comment inside a selector or between property and value keeps the statement as written.
- **Reproduce (needs a source checkout):** `scripts/parity.sh format prettier-css`, `scripts/parity.sh coverage stylelint`, `scripts/parity.sh coverage stylelint-scss`, `scripts/parity.sh coverage stylelint-less`.

## JSON / JSONC

<!-- generated:measured-json-jsonc -->
- **Measured:** Ready.
  - Format, Prettier: 99% of the files identical (3,188 of 3,216 files).
  - Format, Biome: 99% of the files identical (3,188 of 3,212 files).
  - Lint, eslint-plugin-jsonc: 100% of its findings also reported (14 of 14 findings).
<!-- /generated -->

- Covers JSONC. Config section: `json`.
- **Formatter:** Prettier 3.9.10 on the JSON files of SchemaStore and the VS Code extension samples. Biome on the same files.
- **Linter:** eslint-plugin-jsonc recommended: on the corpus only `no-useless-escape` fires.
- **Presets:** `json/prettier-compat`, `json/biome`, `json/dprint`, `json/eslint-plugin-jsonc-recommended`.
- **Options:** `collapse` (`preserve`, `always`, `never`, `opening_break`), `trailing_commas` (JSONC), `dialect` (`auto`, `json`, `jsonc`), `bracket_spacing`. Default `line_width` is 80.
- **Differences:**
  - An array of numbers that does not fit gets one number per line; Prettier fills lines.
  - An array of arrays that fits stays on one line; Prettier breaks the outer array.
  - A `.json` file with a trailing comma is refused (the `jsonc` dialect allows it).
- **Reproduce (needs a source checkout):** `scripts/parity.sh format prettier`, `scripts/parity.sh format biome`, `scripts/parity.sh coverage eslint-jsonc`.

## Markdown

<!-- generated:measured-markdown -->
- **Measured:** Ready.
  - Format, Prettier: 92% of the files identical (96 of 104 files).
  - Lint, markdownlint: 99% of its findings also reported (1,965 of 1,985 findings).
<!-- /generated -->

- **Formatter:** Prettier 3.9.10. Corpus: the repository docs and the dotnet coding guidelines. Small corpus.
- **Linter:** markdownlint.
- **Presets:** `markdown/prettier-compat`, `markdown/markdownlint`.
- **Options:** `embedded_language_formatting` (formats fenced code with that language's formatter).
- **Differences:**
  - Trailing blanks inside code blocks stay. Prettier removes them.
  - A block with a tab is kept as written.
  - A table of contents with unusual list indent is kept as written.
- **Reproduce (needs a source checkout):** `scripts/parity.sh format prettier`, `scripts/parity.sh coverage markdownlint`.

## Python

<!-- generated:measured-python -->
- **Measured:** Ready.
  - Format, ruff format: 97% of the files identical (1,060 of 1,085 files).
  - Lint, ruff: 94% of its findings also reported (4,132 of 4,367 findings).
<!-- /generated -->

- Config section: `python`.
- **Formatter:** ruff format 0.17.4 (Black style).
  - Corpus: real-world projects, Black's tests and Ruff's formatter fixtures.
- **Linter:** ruff default rules.
- **Presets:** `python/ruff`.
- **Options:** `quote_style`, `magic_trailing_comma` (`respect` or `ignore`), `target_version` (`3.9` to `3.15`). Default `line_width` is 88.
- **Differences:**
  - Code in docstrings is not formatted.
  - Comments in unusual places stay where the source has them, not where ruff puts them.
  - Quotes inside f-string replacement fields are not changed.
  - `# fmt: skip` on compound headers is not honored.
- **Reproduce (needs a source checkout):** `scripts/parity.sh format ruff`, `scripts/parity.sh coverage ruff`.

## YAML

<!-- generated:measured-yaml -->
- **Measured:** Ready.
  - Format, Prettier: 94% of the files identical (808 of 854 files).
  - Lint, yamllint: 98% of its findings also reported (2,240 of 2,274 findings).
<!-- /generated -->

- Config section: `yaml`.
- **Formatter:** Prettier 3.9.10. Corpus: YAML files of real projects (awesome-compose, the Kubernetes website, GitHub starter workflows, actionlint). The yaml-test-suite valid cases parse, round-trip and keep their data model.
- **Linter:** yamllint.
- **Presets:** `yaml/prettier-compat`, `yaml/yamllint`.
- **Options:** `quote_style`, `sequence_indent`, `document_start`, `document_end`, `max_blank_lines`, `comment_spacing`, `bracket_spacing`. Default `line_width` is 80.
- **Differences:**
  - Long scalars and comments are not re-wrapped.
  - A broken flow sequence has no trailing comma; Prettier writes one.
  - A comment before a dedented item takes the next line's column; Prettier keeps its own.
- **Reproduce (needs a source checkout):** `scripts/parity.sh format prettier`, `scripts/parity.sh coverage yamllint`.

## Shell

<!-- generated:measured-shell -->
- **Measured:** Ready.
  - Format, shfmt: 86% of the files identical (70 of 81 files).
  - Lint, ShellCheck: 86% of its findings also reported (201 of 232 findings).
<!-- /generated -->

- **Formatter:** shfmt 3.14.1. Corpus: install scripts of dotnet, nvm, rustup, docker, Homebrew and others.
- **Linter:** ShellCheck 0.11.0.
- **Presets:** `shell/shellcheck`, `shell/shfmt`.
- **Options:** `max_blank_lines`, `comment_column`, `heredoc_indent`, `case_last_terminator`.
- **Differences:**
  - Backticks stay. shfmt rewrites them to `$(...)`.
  - Without the `shell/shfmt` preset a last `case` arm without `;;` stays, tabs in `<<-` heredocs are kept and trailing comments keep their blanks.
- **Reproduce (needs a source checkout):** `scripts/parity.sh format shfmt`, `scripts/parity.sh coverage shellcheck`.

## Dockerfile

<!-- generated:measured-dockerfile -->
- **Measured:** Ready.
  - Format, stable: 100% of the files format stably, by the rules of the formatter (211 of 211 files).
  - Lint, hadolint: 91% of its findings also reported (685 of 751 findings).
<!-- /generated -->

- The formatter fixes keyword case and blank lines. No reference formatter exists: the format column is the stable share.
- **Linter:** hadolint.
- **Presets:** `dockerfile/hadolint`.
- **Options:** `max_blank_lines`.
- **Differences:** a variable as `USER` is accepted whether or not an `ARG` defines it. hadolint reports it.
- **Reproduce (needs a source checkout):** `scripts/parity.sh coverage hadolint` (lint), `scripts/parity.sh stability dockerfile` (stable).

## SQL

<!-- generated:measured-sql -->
- **Measured:** Ready.
  - Format, sqlfluff: 85% of the files identical (703 of 825 files).
  - Lint, sqlfluff: 88% of its findings also reported (100,738 of 113,542 findings).
<!-- /generated -->

- Dialects: `ansi`, `postgres`, `mysql`, `tsql` (option `dialect`).
- **Formatter:** sqlfluff 4.5.0 (`fix --rules layout,capitalisation`). Corpus: 971 real files of 18 permissively licensed projects (Hive, Ranger, Kestra, Temporal, OpenFGA, DolphinScheduler, Spring, pgvector, Pagila, BenchBase, DataFusion, SQL Server DBA scripts), the dialect set per project folder; sqlfluff's own dialect fixtures and PostgreSQL's regression inputs are measured and ratcheted, not in the README number.
- **Linter:** sqlfluff on the same corpus.
- **Presets:** `sql/sqlfluff`.
- **Options:** `dialect`, `comma_position`, `keyword_case`, `identifier_case`, `function_case`, `literal_case`, `type_case`. Default `line_width` is 80.
- **Differences:**
  - Files the parser does not accept are refused.
  - Layout of some constructs is not written: expressions, joins, CASE, CTEs, comments. Those parts stay as written.
- **Reproduce (needs a source checkout):** `scripts/parity.sh format sqlfluff`, `scripts/parity.sh coverage sqlfluff`.

## Java

<!-- generated:measured-java -->
- **Measured:** Ready.
  - Format, google-java-format: 91% of the files identical (4,898 of 5,337 files).
  - Lint, Checkstyle: 88% of its findings also reported (53,149 of 59,973 findings).
<!-- /generated -->

- **Formatter:** google-java-format 2.0.0, run with `--skip-javadoc-formatting --skip-removing-unused-imports --skip-reflowing-long-strings`. Corpus: Guava, Gson, Error Prone, Commons Lang.
- **Linter:** Checkstyle.
- **Presets:** `java/google`.
- **Options:** `member_chain_*`, `binary_*`, `assignment_*` and `conditional_*` layout options. Default `line_width` is 100.
- **Differences:**
  - Long line comments are not re-wrapped.
  - A method chain inside a ternary is indented differently.
  - `@A()` keeps its parentheses.
- **Reproduce (needs a source checkout):** `scripts/parity.sh format google-java-format`, `scripts/parity.sh coverage checkstyle`.

## GitHub Actions

<!-- generated:measured-github-actions -->
- **Measured:** Ready.
  - Format, stable: 99% of the files format stably, by the rules of the formatter (393 of 395 files).
  - Lint, actionlint: 87% of its findings also reported (627 of 721 findings).
<!-- /generated -->

- Files use the YAML formatter. `run:` scripts are checked as Shell.
- **Linter:** actionlint. Only errors and `script-injection` are on by default.
- **Presets:** `github-actions/actionlint`.
- **Options:** the YAML options. Config section: `github-actions`, on top of `yaml`.
- **Reproduce (needs a source checkout):** `scripts/parity.sh coverage actionlint` (lint), `scripts/parity.sh stability github-actions` (stable).

## C#

<!-- generated:measured-c -->
- **Measured:** Ready.
  - Format, CSharpier: 85% of the files identical (6,521 of 7,620 files).
  - Lint, StyleCop: 87% of its findings also reported (357,012 of 409,179 findings).
  - Lint, Roslynator: 0% of its findings also reported (40 of 11,111 findings).
<!-- /generated -->

- Needs the .NET 10 runtime. Config section: `csharp`.
- **Formatter:** CSharpier 1.3.0. Corpus: aspnetcore, Roslyn and runtime; the files CSharpier leaves alone are not compared.
- **Linter:** StyleCop and Roslynator rules. StyleCop (1.2.0-beta.556, run as an analyzer driver over the corpus) is compared for the rules it ships on. Roslynator (Roslynator.Analyzers 5.0.1, the same driver with the framework references): few of its default rules have a Sudsy equivalent yet; the findings of 8 rules that change without the references of the projects are left out.
- **Presets:** `csharp/csharpier-compat`, `csharp/stylecop`, `csharp/roslynator`, `csharp/dotnet-runtime`, `csharp/google`, `csharp/unity`.
- **Options:** `brace_newline`, `brace_same_line`, `trailing_commas`, `sort_usings`, `member_chain_*`, `preprocessor_symbols`, `derive_symbol_sets`, `blank_lines_between_members`, `empty_braces`. Default `line_width` is 120.
- **Differences:**
  - Lists that differ by a comma, mostly initializers after wrapped arguments.
  - Comment placement and text next to `#if` directives .
  - Wrapped arguments and member chains .
  - CSharpier formats interpolated string holes. Sudsy does not.
- **Reproduce (needs a source checkout):** `scripts/parity.sh format csharpier`, `scripts/parity.sh format dotnet-format`, `scripts/parity.sh coverage stylecop`. All need the .NET SDK.

## PowerShell

<!-- generated:measured-powershell -->
- **Measured:** Ready.
  - Format, Invoke-Formatter: 92% of the files identical (194 of 209 files).
  - Lint, PSScriptAnalyzer: 87% of its findings also reported (933 of 1,071 findings).
<!-- /generated -->

- **Formatter:** Invoke-Formatter (PSScriptAnalyzer 1.25.0). Trailing blanks are ignored in the comparison.
- **Linter:** PSScriptAnalyzer.
- **Presets:** `powershell/psscriptanalyzer`.
- **Options:** `max_blank_lines` (default 10).
- **Differences:**
  - Invoke-Formatter outdents a script block after a backtick continuation. Sudsy keeps its indentation.
  - The module knows types of scalar variables. Sudsy has no types, so some null-comparison findings differ.
- **Reproduce (needs a source checkout):** `scripts/parity.sh format invoke-formatter`, `scripts/parity.sh coverage psscriptanalyzer`. Needs PowerShell.

## Makefile

<!-- generated:measured-makefile -->
- **Measured:** Ready.
  - Format, stable: 100% of the files format stably, by the rules of the formatter (59 of 59 files).
  - Lint, checkmake: 100% of its findings also reported (349 of 349 findings).
<!-- /generated -->

- The formatter normalizes assignment spacing and blank lines. No reference formatter exists: the format column is the stable share.
- **Linter:** checkmake.
- **Presets:** `makefile/checkmake`.
- **Options:** `space_around_assignment`, `space_before_colon`, `max_blank_lines` (default 2).
- **Differences:** `A+=b`, `A?=b` and `A!=b` without a blank before the operator are left as written.
- **Reproduce (needs a source checkout):** `scripts/parity.sh coverage checkmake` (lint), `scripts/parity.sh stability makefile` (stable).

## TOML

<!-- generated:measured-toml -->
- **Measured:** Ready.
  - Format, taplo: 98% of the files identical (1,282 of 1,307 files).
  - Lint: 22 rules, no reference tool to compare with.
<!-- /generated -->

- **Formatter:** taplo 0.10. Corpus: the .toml files of rust-analyzer, Bevy, Tokio, clap, serde, uv and Cargo (1,310 files); toml-test and the repository fixtures are listed apart.
- **Linter:** none from a reference tool. Duplicate keys are reported.
- **Presets:** `toml/taplo-compat` (`collapse: always`, `max_blank_lines: 2`, `align_comments: true`).
- **Options:** `collapse`, `trailing_commas`, `align_comments`, `max_blank_lines`, `bracket_spacing`, `array_bracket_spacing`.
- **Differences:**
  - Taplo keeps CRLF inside multi-line strings. Sudsy writes LF.
  - Taplo expands arrays inside long inline tables. Sudsy breaks only what does not fit.
  - Taplo rejects some valid files. Sudsy formats them.
- **Reproduce (needs a source checkout):** `scripts/parity.sh format taplo`.

## XML

<!-- generated:measured-xml -->
- **Measured:** Ready.
  - Format, CSharpier XML: 100% of the files identical (321 of 321 files).
  - Lint: 24 rules, no reference tool to compare with.
<!-- /generated -->

- Covers `.csproj`, `.props`, `.config`.
- **Formatter:** CSharpier XML on the aspnetcore project files (372 files, 49 of them reviewed as CSharpier width accounting and left out of the share).
- **Linter:** none.
- **Presets:** `xml/csharpier-compat`.
- **Options:** `max_blank_lines`. Default `line_width` is 100.
- **Differences:** line width is counted differently in some `.resx` files.
- **Reproduce (needs a source checkout):** `scripts/parity.sh format csharpier-xml`.

## Go

<!-- generated:measured-go -->
- **Measured:** Lint only: Ready.
  - Format: Sudsy does not format the language.
  - Lint, revive: 97% of its findings also reported (46,327 of 47,483 findings).
<!-- /generated -->
- Files: `*.go`.
- **Formatter:** none yet; files stay as written. gofmt ships with Go and is the reference.
- **Linter:** revive 1.17.1 with its default rule set; the catalog is [ai/parity/revive.yaml](ai/parity/revive.yaml).
- **Presets:** `go/revive`.
- **Reproduce (needs a source checkout):** `scripts/parity.sh coverage revive`.

## CMake

<!-- generated:measured-cmake -->
- **Measured:** Ready.
  - Format, gersemi: 90% of the files identical (8,187 of 9,038 files).
  - Lint, cmake-lint: 97% of its findings also reported (124,892 of 128,501 findings).
<!-- /generated -->
- Files: `CMakeLists.txt` and `*.cmake`.
- **Formatter:** gersemi 0.29.2 (its defaults: 80 columns, 4 spaces, `favour-inlining`). Corpus: LLVM, CMake, Qt, VTK, OpenCV, extra-cmake-modules and protobuf.
- **Options:** `leading_argument_commands` (commands whose first argument stays on the line when the call breaks; default `[set]`).
- **Differences:**
  - Keywords that only the prose of CMake's help defines are not in the command table, so those calls follow the generic layout.
  - Conditions break before `AND` and `OR` only; gersemi also breaks nested groups.
  - Commands the table does not know (user functions and macros) are kept as written.
- **Reproduce (needs a source checkout):** `scripts/parity.sh format gersemi`, `scripts/parity.sh coverage cmake-lint`.
- **Linter:** cmake-lint (cmakelang 0.6.13) with its defaults; the 35 codes are in [ai/parity/cmake-lint.yaml](ai/parity/cmake-lint.yaml).
- **Presets:** `cmake/cmake-lint` turns on the rules Sudsy has for it (they are off by default).

## Kotlin

<!-- generated:measured-kotlin -->
- **Measured:** Ready.
  - Format, ktlint: 85% of the files identical (4,924 of 5,776 files).
  - Lint, ktlint: 92% of its findings also reported (109,702 of 118,836 findings).
<!-- /generated -->

- **Formatter:** ktlint 1.8.0 (`ktlint_official`). Corpus: coroutines, ktor, nowinandroid, ktlint.
- **Linter:** ktlint.
- **Presets:** `kotlin/ktlint`.
- **Options:** `trailing_commas`, `function_signature_wrap_min_parameters`, `class_signature_wrap_min_parameters`, `member_chain_style`. Default `line_width` is 140.
- **Differences:**
  - ktlint rewrites not done: modifier order and comments in the modifier list.
  - Operator chains follow the source breaks.
  - Values ktlint moves below `=` differ in some cases.
- **Reproduce (needs a source checkout):** `scripts/parity.sh format ktlint`, `scripts/parity.sh coverage ktlint`.

## Terraform (HCL)

<!-- generated:measured-terraform-hcl -->
- **Measured:** Ready.
  - Format, terraform fmt: 99% of the files identical (313 of 316 files).
  - Lint, tflint: 93% of its findings also reported (133 of 143 findings).
<!-- /generated -->

- Config section: `hcl`.
- **Formatter:** terraform fmt 1.17.1. Corpus: terraform-aws-vpc, terraform-aws-eks, Terraform and Packer test data.
- **Linter:** tflint recommended. Few tflint rules exist in Sudsy.
- **Presets:** `hcl/tflint-recommended`.
- **Options:** only the shared ones.
- **Differences:**
  - `terraform fmt` rewrites tokens, for example `list` to `list(any)`. Sudsy keeps tokens.
  - `terraform fmt` rejects files with some semantic errors. Sudsy formats them.
  - Unwrapping `"${x}"` is a lint fix (`deprecated-interpolation`), not part of format.
- **Reproduce (needs a source checkout):** `scripts/parity.sh format terraform`, `scripts/parity.sh coverage tflint`.

## INI / EditorConfig / .gitignore

<!-- generated:measured-ini-editorconfig-gitignore -->
- **Measured:** Ready.
  - Format, stable: 100% of the files format stably, by the rules of the formatter (129 of 129 files).
  - Lint: 31 rules, no reference tool to compare with.
<!-- /generated -->

- Small formatters and a few rules. No reference tool, so no comparison: the format column is the stable share.
- **Options:** `max_blank_lines`. INI adds `space_around_equals`.
- **Rules:** invalid or duplicate entries, for example unknown EditorConfig properties and ignored `.gitignore` patterns.

## Ruby

<!-- generated:measured-ruby -->
- **Measured:** Lint only: Ready.
  - Format: Sudsy does not format the language.
  - Lint, RuboCop: 96% of its findings also reported (7,717 of 8,022 findings).
<!-- /generated -->

- The formatter keeps files as written.
- **Linter:** RuboCop Lint department. Other departments are not covered.
- **Presets:** `ruby/rubocop`.
- **Reproduce (needs a source checkout):** `scripts/parity.sh coverage rubocop`.

## GraphQL

<!-- generated:measured-graphql -->
- **Measured:** Ready.
  - Format, Prettier: 99% of the files identical (123 of 124 files).
  - Lint: 29 rules, no reference tool to compare with.
<!-- /generated -->

- **Presets:** `graphql/graphql-eslint-recommended`.
- **Formatter:** Prettier 3.9.10. Corpus: Prettier tests, graphql-js, GraphiQL, Apollo Federation, graphql-eslint.
- **Linter:** graphql-eslint recommended, the 20 rules that need no schema (6 of them fire on the corpus). Open: `naming-convention` is half covered, `description-style` and `require-deprecation-reason` are missing.
- **Options:** `line_width` (80), `bracket_spacing`.
- **Differences:** the files that differ are listed in the source checkout.
- **Reproduce (needs a source checkout):** `scripts/parity.sh format prettier`, `scripts/parity.sh coverage graphql-eslint`.

## Protobuf

<!-- generated:measured-protobuf -->
- **Measured:** Ready.
  - Format, buf format: 99% of the files identical (319 of 322 files).
  - Lint, buf: 85% of its findings also reported (2,027 of 2,362 findings).
<!-- /generated -->

- **Formatter:** buf format 1.75.0. Corpus: googleapis, protobuf and grpc protos.
- **Linter:** buf STANDARD.
- **Presets:** `protobuf/buf-standard`.
- **Options:** only the shared ones.
- **Differences:** Sudsy keeps empty statements (`;`). buf drops them.
- **Reproduce (needs a source checkout):** `scripts/parity.sh format buf`, `scripts/parity.sh coverage buf`.

## C / C++

<!-- generated:measured-c-c -->
- **Measured:** In progress.
  - Format, clang-format: 69% of the files identical (4,606 of 6,639 files).
  - Lint, clang-tidy: not measured.
  - clang-format: line wrapping is not compared.
<!-- /generated -->

- Layout by `format.rules` (indent, blanks, braces, keyword and operator spacing, comments); source line breaks and `*`/`&` spacing stay as written. There are no lint rules.
- **Parser:** a lexer with a token-level structure. There is no full grammar.
- **Not done:** a clang-format compatible layout. The format number ignores line wrapping (where a statement, a list or an enum breaks, and the indent of the continuation lines); the rest is compared exactly, and the byte-identical share is printed next to it.

## PHP

<!-- generated:measured-php -->
- **Measured:** In progress.
  - Format, PHP-CS-Fixer: 88% of the files identical (9,190 of 10,382 files).
  - Lint, PHPStan: 36% of its findings also reported (340 of 942 findings).
  - PHP-CS-Fixer: measured with the `@PER-CS` rule set (the default of PHP-CS-Fixer 3 is `@PSR12`, which `@PER-CS` contains).
<!-- /generated -->

- Config section: `php`.
- **Formatter:** PHP-CS-Fixer 3.95.31 with `@PER-CS`. Corpus: Laravel, Symfony, WordPress, Composer.
- **Linter:** PHPStan level 0, the mapped rules (`new.static`, `function.inner`, `parameter.implicitlyNullable`, `unset.possiblyHookedProperty`, `variable.undefined`). Sudsy does no type analysis; the rules are on in `php/phpstan` except `missing-return`, which is off (more extras than findings).
- **Presets:** `php/phpstan`.
- **Options:** `trailing_commas`, `declaration_brace`. Default `line_width` is 120.
- **Differences:**
  - Files with inline text (`?>`): PHP-CS-Fixer formats the PHP islands. Sudsy keeps the file as written.
  - One `use` per trait is not split.
  - `else if` to `elseif`, modifier order and import order are not rewritten.
- **Reproduce (needs a source checkout):** `scripts/parity.sh format php-cs-fixer`, `scripts/parity.sh coverage phpstan`.

## Reproduce (needs a source checkout)

Each comparison runs from a clone of the repository. `scripts/parity.sh format <tool>` compares formatting. `scripts/parity.sh coverage <tool>` measures lint findings. Each run downloads the tool and the corpus. The complete list of differences per tool is in `tests/parity/<tool>/differences.yaml` and in the generated sections of the language pages under [ai/formatting/](ai/formatting/js.md). Method: [ai/development/parity.md](ai/development/parity.md).

More: [format options in detail](ai/formatting/presets.md), [lint rule coverage](ai/parity/README.md), [how comparisons run](ai/development/parity.md).
