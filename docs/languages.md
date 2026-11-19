# Languages

Status, measured results, presets and options for every language.

## How to read the numbers

- **Identical:** share of corpus files where `sudsy format` gives the same bytes as the reference formatter. The corpus is real-world code unless the row says otherwise.
- **Stable:** formatting a second time changes no file. "Not measured" means the run did not record it.
- **Findings:** share of the reference linter's findings on the corpus that Sudsy also reports with the named preset. This is recall. False positives are not counted.
- All numbers compare against the reference tool with its default settings, not your project's config. See [keep your current style](configuration.md#keep-your-current-style).
- With no config, lint reports likely bugs only. Findings numbers need the preset. See [configuration](configuration.md#presets).
- **Status:** *ready* means 85% or more identical and stable. *Partial* means it works but differs more, or its lint is below 85% findings. *Lint only* means no reference formatter was compared. *Experimental* means little or no support.
- HTML and PowerShell format at 93%, but are partial: their lint findings (64% and 34%) miss the 85% bar.
- Every number comes from a pinned tool version. Versions are in the sections below.
- The main differences per language are listed in each section. The complete lists live in the source checkout (see [Reproduce](#reproduce-needs-a-source-checkout)).
- Language names in `sudsy.yaml` sections match `sudsy languages list`: `js` (JavaScript and TypeScript), `python`, `hcl`, `shell`, and so on.

## Summary

| Language | Status | Formats like | Identical | Stable | Lints like | Findings |
|---|---|---|---:|---|---|---|---:|
| [JavaScript / TypeScript](#javascript--typescript) | ready | Prettier 3.9.10 | 93% of 2,456 | yes | ESLint, typescript-eslint | 100% |
| [Python](#python) | ready | ruff format 0.17.4 | 93% of 1,085 | yes | ruff | 96% |
| [Protobuf](#protobuf) | ready | buf format 1.75.0 | 99% of 323 | yes | buf STANDARD | 86% |
| [Terraform (HCL)](#terraform-hcl) | ready | terraform fmt 1.17.1 | 98% of 318 | yes | tflint | 20% |
| [GraphQL](#graphql) | ready | Prettier 3.9.10 | 98% of 132 | yes | none | n/a |
| [TOML](#toml) | ready | taplo 0.10 | 95% of 213 | not measured | none | n/a |
| [HTML](#html-vue-svelte-angular) | partial | Prettier 3.9.10 | 93% of 233 | Angular: no; others not measured | html-validate | 64% |
| [PowerShell](#powershell) | partial | Invoke-Formatter | 93% of 211 | not measured | PSScriptAnalyzer | 34% |
| [PHP](#php) | partial | PHP-CS-Fixer 3.95.31 | 88% of 10,382 | yes | PHPStan (`php/phpstan`) | 0% |
| [Markdown](#markdown) | partial | Prettier 3.9.10 | 86% of 64 | not measured | markdownlint | 83% |
| [Java](#java) | partial | google-java-format 2.0.0 | 91% of 5,338 | yes | Checkstyle | 26% |
| [Shell](#shell) | partial | shfmt | 82% of 82 | yes | ShellCheck | 47% |
| [SQL](#sql) | partial | sqlfluff 4.5.0 | 78% of 867 | yes | sqlfluff | 4% |
| [C#](#c) | partial | CSharpier 1.3.0 | 72% of 7,615 | not measured | StyleCop | not measured |
| [Kotlin](#kotlin) | partial | ktlint 1.8.0 | 69% of 5,776 | yes | ktlint | 13% |
| [Vue / Svelte](#html-vue-svelte-angular) | partial | Prettier | 83% / 89% real-world | not measured | eslint-plugin-vue / -svelte | 87% / 100% |
| [YAML](#yaml) | partial | Prettier | not measured | not measured | yamllint | 74% |
| [JSON / JSONC](#json) | partial | Prettier, Biome | not measured | not measured | eslint-plugin-jsonc | not measured |
| [CSS / SCSS / Less](#css-scss-less) | partial | Prettier 3.9.10 | 93% of 533 | yes | stylelint | 13% |
| [XML](#xml) | partial | CSharpier XML | not measured | not measured | none | n/a |
| [Ruby](#ruby) | lint only | none | n/a | n/a | RuboCop (Lint department) | 96% |
| [Dockerfile](#dockerfile) | lint only | none | n/a | n/a | hadolint | 28% |
| [Makefile](#makefile) | lint only | none | n/a | n/a | checkmake | 74% |
| [GitHub Actions](#github-actions) | lint only | none | n/a | n/a | actionlint | 5% |
| [C / C++](#c--c) | experimental | none | n/a | n/a | none | n/a |
| [INI, EditorConfig, .gitignore](#ini-editorconfig-gitignore) | experimental | none | n/a | n/a | none | n/a |

## Options every formatter shares

`line_width`, `indent_size`, `indent_style` (`space` or `tab`), `final_newline`, `end_of_line` (`auto`, `lf`, `crlf`), `bracket_spacing` and `array_bracket_spacing`. A language ignores a shared option it does not declare. Set them once at the top level, or per language. See [configuration](configuration.md#format-options). Each section lists only the options that go beyond these.

## JavaScript / TypeScript

- **Status:** ready. Config section: `js`.
- **Formatter:** Prettier 3.9.10 (`--no-config`).
  - Identical: 2,288 of 2,456 real-world files (93.2%). Stable: yes.
  - Prettier's own tests: 1,115 of 1,595 files (69.9%). Stable: yes.
- **Linter:** ESLint recommended rules at 100.0% and typescript-eslint recommended at 100.0%. Core rules only. No react, import or a11y plugins. No type-aware rules.
- **Presets:** `js/eslint-recommended`, `js/typescript-eslint-recommended`, `js/biome`, `js/biome-recommended`, `js/oxlint`.
- **Options:** `semi`, `quote_style`, `quote_props`, `trailing_commas`, `arrow_parens`, `bracket_spacing`, `bracket_same_line`, `jsx_single_quote`, `member_chain_style`. Default `line_width` is 80.
- **Differences:**
  - A line comment in a class or interface header moves into the body.
  - JSX as the object of a member access keeps its parentheses.
  - Syntax the parser rejects gives a syntax error: pipeline operator, do expressions, Flow, bind `::`.
- **Reproduce (needs a source checkout):** `scripts/parity.sh format prettier-js`, `scripts/parity.sh coverage eslint`, `scripts/parity.sh coverage typescript-eslint`.

## Python

- **Status:** ready. Config section: `python`.
- **Formatter:** ruff format 0.17.4 (Black style).
  - Identical: 1,006 of 1,085 real-world files (92.7%). Stable: yes.
  - Black's tests: 180 of 302. Ruff's formatter fixtures: 96 of 277. Stable in all groups.
- **Linter:** ruff default rules, 95.6%.
- **Presets:** `python/ruff`.
- **Options:** `quote_style`, `magic_trailing_comma` (`respect` or `ignore`), `target_version` (`3.9` to `3.15`). Default `line_width` is 88.
- **Differences:**
  - Code in docstrings is not formatted.
  - Comments in unusual places stay where the source has them, not where ruff puts them.
  - Quotes inside f-string replacement fields are not changed.
  - `# fmt: skip` on compound headers is not honored.
- **Reproduce (needs a source checkout):** `scripts/parity.sh format ruff`, `scripts/parity.sh coverage ruff`.

## Protobuf

- **Status:** ready.
- **Formatter:** buf format 1.75.0. Identical: 320 of 323 files (99.1%). Corpus: googleapis, protobuf and grpc protos. Stable: yes.
- **Linter:** buf STANDARD, 85.9%.
- **Presets:** `protobuf/buf-standard`.
- **Options:** only the shared ones.
- **Differences:** Sudsy keeps empty statements (`;`). buf drops them.
- **Reproduce (needs a source checkout):** `scripts/parity.sh format buf`, `scripts/parity.sh coverage buf`.

## Terraform (HCL)

- **Status:** ready. Config section: `hcl`.
- **Formatter:** terraform fmt 1.17.1. Identical: 312 of 318 files (98.1%). Corpus: terraform-aws-vpc, terraform-aws-eks, Terraform and Packer test data. Stable: yes.
- **Linter:** tflint recommended, 19.6%. Low: few tflint rules exist in Sudsy.
- **Presets:** `hcl/tflint-recommended`.
- **Options:** only the shared ones.
- **Differences:**
  - `terraform fmt` rewrites tokens, for example `list` to `list(any)`. Sudsy keeps tokens.
  - `terraform fmt` rejects files with some semantic errors. Sudsy formats them.
  - Unwrapping `"${x}"` is a lint fix (`deprecated-interpolation`), not part of format.
- **Reproduce (needs a source checkout):** `scripts/parity.sh format terraform`, `scripts/parity.sh coverage tflint`.

## GraphQL

- **Status:** ready. No linter preset.
- **Formatter:** Prettier 3.9.10. Identical: 129 of 132 files (97.7%). Corpus: Prettier tests, graphql-js, GraphiQL, Apollo Federation, graphql-eslint. Stable: yes.
- **Linter:** a few schema-free graphql-eslint rules exist. Coverage is not measured.
- **Options:** `line_width` (80), `bracket_spacing`.
- **Differences:** 3 files differ. They are listed in the source checkout.
- **Reproduce (needs a source checkout):** `scripts/parity.sh format prettier`.

## TOML

- **Status:** ready.
- **Formatter:** taplo 0.10. Identical: 203 of 213 files (95.3%). Corpus: toml-test valid files and fixtures. Stable: not measured.
- **Linter:** none from a reference tool. Duplicate keys are reported.
- **Presets:** `toml/taplo-compat` (`collapse: always`, `max_blank_lines: 2`, `align_comments: true`).
- **Options:** `collapse`, `trailing_commas`, `align_comments`, `max_blank_lines`, `bracket_spacing`, `array_bracket_spacing`.
- **Differences:**
  - Taplo keeps CRLF inside multi-line strings. Sudsy writes LF.
  - Taplo expands arrays inside long inline tables. Sudsy breaks only what does not fit.
  - Taplo rejects some valid files. Sudsy formats them.
- **Reproduce (needs a source checkout):** `scripts/parity.sh format taplo`.

## HTML, Vue, Svelte, Angular

- **Status:** partial. One parser serves all four. Config sections: `html`, `vue`, `svelte`, `angular`.
- **Formatter:** Prettier 3.9.10 (Svelte: with prettier-plugin-svelte 3.5.2).

| Input | Identical |
|---|---:|
| HTML, real-world | 216 of 233 (92.7%) |
| HTML, Prettier tests | 129 of 144 (89.6%) |
| Vue, real-world | 782 of 941 (83.1%) |
| Vue, Prettier tests | 56 of 80 (70.0%) |
| Svelte, real-world | 941 of 1,055 (89.2%) |
| Svelte, plugin samples | 51 of 76 (67.1%) |
| Angular templates (`--parser angular`), components | 681 of 684 |
| Angular, Prettier tests | 56 of 80 |

- **Stable:** Angular is not: 2 component files and 1 Prettier-test file change on a second format. Not measured for the others.
- **Linters:** html-validate 63.7%, htmlhint 5.0%, html-eslint 0.3%, eslint-plugin-vue 87.2%, eslint-plugin-svelte 100.0%. Angular: 3 of 37 angular-eslint rules, no coverage run.
- **Presets:** `html/html-validate-recommended`, `html/html-validate-standard`, `html/htmlhint`, `html/html-eslint-recommended`, `html/eslint-plugin-vue-recommended`, `html/eslint-plugin-svelte-recommended`.
- **Options:** `whitespace_sensitivity`, `bracket_same_line`, `single_attribute_per_line`, `embedded_language_formatting`. Vue and Svelte add `indent_script_and_style`. Default `line_width` is 80.
- **Differences:** Vue and Svelte differ most. The named gaps are in the source checkout.
- **Reproduce (needs a source checkout):** `scripts/parity.sh format prettier-html`, `scripts/parity.sh format prettier-angular`, `scripts/parity.sh coverage html-validate`.

## PowerShell

- **Status:** partial.
- **Formatter:** Invoke-Formatter (PSScriptAnalyzer 1.25.0). Identical: 196 of 211 files (92.9%), trailing blanks ignored. Stable: not measured.
- **Linter:** PSScriptAnalyzer, 34.4%.
- **Presets:** `powershell/psscriptanalyzer`.
- **Options:** `max_blank_lines` (default 10).
- **Differences:**
  - Invoke-Formatter outdents a script block after a backtick continuation. Sudsy keeps its indentation.
  - The module knows types of scalar variables. Sudsy has no types, so some null-comparison findings differ.
- **Reproduce (needs a source checkout):** `scripts/parity.sh format invoke-formatter`, `scripts/parity.sh coverage psscriptanalyzer`. Needs PowerShell.

## PHP

- **Status:** partial. Config section: `php`.
- **Formatter:** PHP-CS-Fixer 3.95.31 with `@PER-CS`. Identical: 9,189 of 10,382 files (88.5%). Corpus: Laravel, Symfony, WordPress, Composer. Stable: yes.
- **Linter:** PHPStan mapping exists, 0.0%. Sudsy does no type analysis.
- **Presets:** `php/phpstan`.
- **Options:** `trailing_commas`, `declaration_brace`. Default `line_width` is 120.
- **Differences:**
  - About 790 files with inline text (`?>`): PHP-CS-Fixer formats the PHP islands. Sudsy keeps the file as written.
  - One `use` per trait is not split.
  - `else if` to `elseif`, modifier order and import order are not rewritten.
- **Reproduce (needs a source checkout):** `scripts/parity.sh format php-cs-fixer`, `scripts/parity.sh coverage phpstan`.

## Markdown

- **Status:** partial.
- **Formatter:** Prettier 3.9.10. Identical: 55 of 64 files (85.9%). Corpus: the repository docs and the dotnet coding guidelines. Small corpus. Stable: not measured.
- **Linter:** markdownlint, 82.7%.
- **Presets:** `markdown/prettier-compat`, `markdown/markdownlint`.
- **Options:** `embedded_language_formatting` (formats fenced code with that language's formatter).
- **Differences:**
  - Trailing blanks inside code blocks stay. Prettier removes them.
  - A block with a tab is kept as written.
  - A table of contents with unusual list indent is kept as written.
- **Reproduce (needs a source checkout):** `scripts/parity.sh format prettier`, `scripts/parity.sh coverage markdownlint`.

## Java

- **Status:** partial.
- **Formatter:** google-java-format 2.0.0, run with `--skip-javadoc-formatting --skip-removing-unused-imports --skip-reflowing-long-strings`. Identical: 4,888 of 5,338 files (91.6%). Corpus: Guava, Gson, Error Prone, Commons Lang. Stable: yes.
- **Linter:** Checkstyle, 25.5%.
- **Presets:** `java/google`.
- **Options:** `member_chain_*`, `binary_*`, `assignment_*` and `conditional_*` layout options. Default `line_width` is 100.
- **Differences:**
  - Long line comments are not re-wrapped.
  - A method chain inside a ternary is indented differently.
  - `@A()` keeps its parentheses.
- **Reproduce (needs a source checkout):** `scripts/parity.sh format google-java-format`, `scripts/parity.sh coverage checkstyle`.

## Shell

- **Status:** partial.
- **Formatter:** shfmt 3.14.1. Identical: 67 of 82 files (81.7%). Corpus: install scripts of dotnet, nvm, rustup, docker, Homebrew and others. Stable: yes.
- **Linter:** ShellCheck 0.11.0, 47.0%.
- **Presets:** `shell/shellcheck`.
- **Options:** `max_blank_lines`.
- **Differences:**
  - Backticks stay. shfmt rewrites them to `$(...)`.
  - A last `case` arm without `;;` stays.
  - Tabs in `<<-` heredocs are kept.
- **Reproduce (needs a source checkout):** `scripts/parity.sh format shfmt`, `scripts/parity.sh coverage shellcheck`.

## SQL

- **Status:** partial. Dialects: `ansi`, `postgres`, `mysql`, `tsql` (option `dialect`).
- **Formatter:** sqlfluff 4.5.0 (`fix --rules layout,capitalisation`). Identical: 678 of 867 files (78.2%). Stable: yes.
- **Linter:** sqlfluff, 4.4%.
- **Presets:** `sql/sqlfluff`.
- **Options:** `dialect`, `comma_position`, `keyword_case`, `identifier_case`, `function_case`, `literal_case`, `type_case`. Default `line_width` is 80.
- **Differences:**
  - Files the parser does not accept are refused.
  - Layout of some constructs is not written: expressions, joins, CASE, CTEs, comments. Those parts stay as written.
- **Reproduce (needs a source checkout):** `scripts/parity.sh format sqlfluff`, `scripts/parity.sh coverage sqlfluff`.

## C#

- **Status:** partial. Needs the .NET 10 runtime. Config section: `csharp`.
- **Formatter:** CSharpier 1.3.0. Identical: 5,488 of 7,615 files (72.1%). Corpus: 7,659 files, of which 7,615 are compared (CSharpier leaves the rest alone). Split: aspnetcore (2,449 of 3,122), Roslyn (1,820 of 2,858), runtime (1,219 of 1,679). Stable: not measured.
- **Linter:** StyleCop and Roslynator rules exist: 120 of 197 StyleCop rules covered. Findings recall: not measured, because the corpus does not compile.
- **Presets:** `csharp/csharpier-compat`, `csharp/stylecop`, `csharp/dotnet-runtime`, `csharp/google`, `csharp/unity`.
- **Options:** `brace_newline`, `brace_same_line`, `trailing_commas`, `sort_usings`, `member_chain_*`, `preprocessor_symbols`, `derive_symbol_sets`, `blank_lines_between_members`, `empty_braces`. Default `line_width` is 120.
- **Differences:**
  - Lists that differ by a comma, mostly initializers after wrapped arguments.
  - Comment placement and text next to `#if` directives (252 files).
  - Wrapped arguments and member chains (299 files).
  - CSharpier formats interpolated string holes. Sudsy does not.
- **Reproduce (needs a source checkout):** `scripts/parity.sh format csharpier`, `scripts/parity.sh format dotnet-format`. Both need the .NET SDK.

## Kotlin

- **Status:** partial.
- **Formatter:** ktlint 1.8.0 (`ktlint_official`). Identical: 4,009 of 5,776 files (69%). Corpus: coroutines, ktor, nowinandroid, ktlint. Stable: yes.
- **Linter:** ktlint, 13.2%.
- **Presets:** `kotlin/ktlint`.
- **Options:** `trailing_commas`, `function_signature_wrap_min_parameters`, `class_signature_wrap_min_parameters`, `member_chain_style`. Default `line_width` is 140.
- **Differences:**
  - ktlint rewrites are not done: `: Unit`, `;`, modifier order, `//x` to `// x`.
  - Operator chains follow the source breaks.
  - Values ktlint moves below `=` differ in some cases.
- **Reproduce (needs a source checkout):** `scripts/parity.sh format ktlint`, `scripts/parity.sh coverage ktlint`.

## YAML

- **Status:** partial. Config section: `yaml`.
- **Formatter:** Prettier. No corpus count recorded: not measured. Stable: not measured. The yaml-test-suite runs: 308 valid cases parse and round-trip, and 279 of 279 data models are equal.
- **Linter:** yamllint, 73.8%.
- **Presets:** `yaml/prettier-compat`, `yaml/yamllint`.
- **Options:** `quote_style`, `sequence_indent`, `document_start`, `document_end`, `max_blank_lines`, `comment_spacing`, `bracket_spacing`. Default `line_width` is 80.
- **Differences:** long scalars and comments are not re-wrapped.
- **Reproduce (needs a source checkout):** `scripts/parity.sh coverage yamllint`.

## JSON

- **Status:** partial. Covers JSONC. Config section: `json`.
- **Formatter:** Prettier and Biome, on fixtures only. No real-world corpus: not measured. Stable: not measured.
- **Linter:** eslint-plugin-jsonc: 36 of 44 rules in the catalog. Recall: not measured.
- **Presets:** `json/prettier-compat`, `json/biome`, `json/dprint`.
- **Options:** `collapse` (`preserve`, `always`, `never`, `opening_break`), `trailing_commas` (JSONC), `dialect` (`auto`, `json`, `jsonc`), `bracket_spacing`. Default `line_width` is 80.
- **Differences:** not measured on real-world files.
- **Reproduce (needs a source checkout):** `scripts/parity.sh format prettier`, `scripts/parity.sh format biome`.

## CSS, SCSS, Less

- **Status:** partial. Config section: `css` (also `scss`, `less`).
- **Formatter:** Prettier 3.9.10 (`--no-config`). Identical: real-world 494 of 533 (92.7%); Prettier's own tests CSS 108 of 154, SCSS 78 of 90, Less 34 of 40. Stable: yes on the conformance corpus (Bootstrap, Bulma and others).
- **Linter:** stylelint standard, 94.1%. SCSS and Less plugins: not measured.
- **Presets:** `css/stylelint-recommended`, `css/stylelint-standard`.
- **Options:** `single_quote`, `max_blank_lines`. Default `line_width` is 80.
- **Differences:**
  - Some SCSS maps and `@include` with comments stay as written.
  - Less mixin calls with `{ }` arguments stay as written.
  - A comment inside a selector or between property and value keeps the statement as written.
- **Reproduce (needs a source checkout):** `scripts/parity.sh format prettier-css`, `scripts/parity.sh coverage stylelint`.

## XML

- **Status:** partial. Covers `.csproj`, `.props`, `.config`.
- **Formatter:** CSharpier XML on 372 aspnetcore project files and fixtures. Identical count: not measured. Stable: not measured.
- **Linter:** none.
- **Presets:** `xml/csharpier-compat`.
- **Options:** `max_blank_lines`. Default `line_width` is 100.
- **Differences:** line width is counted differently in some `.resx` files.
- **Reproduce (needs a source checkout):** `scripts/parity.sh format csharpier-xml`.

## Ruby

- **Status:** lint only. The formatter keeps files as written.
- **Linter:** RuboCop Lint department, 96.2%. Other departments are not covered.
- **Presets:** `ruby/rubocop`.
- **Reproduce (needs a source checkout):** `scripts/parity.sh coverage rubocop`.

## Dockerfile

- **Status:** lint only in practice. The formatter fixes keyword case and blank lines. No reference formatter was compared.
- **Linter:** hadolint, 27.6%.
- **Presets:** `dockerfile/hadolint`.
- **Options:** `max_blank_lines`.
- **Differences:** a variable as `USER` is accepted whether or not an `ARG` defines it. hadolint reports it.
- **Reproduce (needs a source checkout):** `scripts/parity.sh coverage hadolint`.

## Makefile

- **Status:** lint only in practice. The formatter normalizes assignment spacing and blank lines. No reference formatter was compared.
- **Linter:** checkmake, 73.5%.
- **Presets:** `makefile/checkmake`.
- **Options:** `space_around_assignment`, `space_before_colon`, `max_blank_lines` (default 2).
- **Differences:** `A+=b`, `A?=b` and `A!=b` without a blank before the operator are left as written.
- **Reproduce (needs a source checkout):** `scripts/parity.sh coverage checkmake`.

## GitHub Actions

- **Status:** lint only in practice. Files use the YAML formatter. `run:` scripts are checked as Shell.
- **Linter:** actionlint, 5.4%. Only errors and `script-injection` are on by default.
- **Presets:** `github-actions/actionlint`.
- **Options:** the YAML options. Config section: `github-actions`, on top of `yaml`.
- **Reproduce (needs a source checkout):** `scripts/parity.sh coverage actionlint`.

## C / C++

- **Status:** experimental. Files are kept as written. There are no lint rules.
- **Parser:** a lexer with a token-level structure. There is no full grammar.
- **Not done:** a clang-format compatible layout.

## INI, EditorConfig, .gitignore

- **Status:** experimental. Small formatters and a few rules. No reference tool, so no comparison.
- **Options:** `max_blank_lines`. INI adds `space_around_equals`.
- **Rules:** invalid or duplicate entries, for example unknown EditorConfig properties and ignored `.gitignore` patterns.

## Language packs

These languages are small downloads, fetched with `sudsy languages add`: editorconfig, gitignore, graphql, hcl, ini, markdown, powershell, protobuf, shell, toml, xml, dockerfile and makefile. Every other language is built in. Until you fetch a pack, its files are skipped with a warning. See [configuration](configuration.md#language-packs).

## Reproduce (needs a source checkout)

Each comparison runs from a clone of the repository. `scripts/parity.sh format <tool>` compares formatting. `scripts/parity.sh coverage <tool>` measures lint findings. Each run downloads the tool and the corpus. The complete list of differences per tool is in `tests/parity/<tool>/differences.yaml` and in the generated sections of the language pages under [ai/formatting/](ai/formatting/js.md). Method: [ai/development/parity.md](ai/development/parity.md).

More: [format options in detail](ai/formatting/presets.md), [lint rule coverage](ai/parity/README.md), [how comparisons run](ai/development/parity.md).
