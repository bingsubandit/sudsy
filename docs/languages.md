# Languages

Corpora, tool versions, presets, options and differences for every language. Status and numbers: the table in the [README](../README.md#language-support).

## How to read the numbers

The status and the headline numbers of every language are in the generated table "Language support" of the [README](../README.md#language-support); this page holds what the table leaves out: corpora, tool versions, presets, options and the main differences.

- **Format:** share of corpus files where `sudsy format` gives the same bytes as the reference formatter. The corpus is real-world code unless the section says otherwise.
- **Lint:** share of the reference linter's findings on the corpus that Sudsy also reports with the named preset, on the same line. This is recall. False positives are not counted.
- All numbers compare against the reference tool with its default settings, not your project's config. See [keep your current style](configuration.md#keep-your-current-style).
- With no config, lint reports likely bugs only. Lint numbers need the preset. See [configuration](configuration.md#presets).
- A lint column with no reference linter shows the number of rules Sudsy has: every rule available to the language, its own and the shared ones, counted from the rule registry. It does not decide `Ready`.
- A format column with no reference formatter shows `stable`: the share of the language's corpus files that parse, format with the meaning check passing, format to the same text a second time and are laid out by the formatter's rules, not copied through. It decides `Ready` like a reference tool's number (at least 85%).
- Both numbers leave out only reviewed intentional differences (`tool-wrong`, `invalid-input`, `wontfix`). The lists are in `tests/parity/<tool>/differences.yaml` and [ai/parity/README.md](ai/parity/README.md).
- Reproduce a number: `scripts/parity.sh <tool>` (format), `scripts/parity.sh coverage <tool>` (lint), `scripts/parity.sh stability <group>` (stable).
- Every number comes from a pinned tool version. Versions are in the sections below.
- Language names in `sudsy.yaml` sections match `sudsy languages list`: `js` (JavaScript and TypeScript), `python`, `hcl`, `shell`, and so on.

## Options every formatter shares

`line_width`, `indent_size`, `indent_style` (`space` or `tab`), `final_newline`, `end_of_line` (`auto`, `lf`, `crlf`), `bracket_spacing` and `array_bracket_spacing`. A language ignores a shared option it does not declare. Set them once at the top level, or per language. See [configuration](configuration.md#format-options). Each section lists only the options that go beyond these.

## JavaScript / TypeScript

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

## Python

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

## Protobuf

- **Formatter:** buf format 1.75.0. Corpus: googleapis, protobuf and grpc protos.
- **Linter:** buf STANDARD.
- **Presets:** `protobuf/buf-standard`.
- **Options:** only the shared ones.
- **Differences:** Sudsy keeps empty statements (`;`). buf drops them.
- **Reproduce (needs a source checkout):** `scripts/parity.sh format buf`, `scripts/parity.sh coverage buf`.

## Terraform (HCL)

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

## GraphQL

- **Presets:** `graphql/graphql-eslint-recommended`.
- **Formatter:** Prettier 3.9.10. Corpus: Prettier tests, graphql-js, GraphiQL, Apollo Federation, graphql-eslint.
- **Linter:** graphql-eslint recommended, the 20 rules that need no schema (6 of them fire on the corpus). 53.7% of its findings (117 of 218): `naming-convention` is half covered, `description-style` and `require-deprecation-reason` are missing.
- **Options:** `line_width` (80), `bracket_spacing`.
- **Differences:** the files that differ are listed in the source checkout.
- **Reproduce (needs a source checkout):** `scripts/parity.sh format prettier`, `scripts/parity.sh coverage graphql-eslint`.

## TOML

- **Formatter:** taplo 0.10. Corpus: toml-test valid files and fixtures.
- **Linter:** none from a reference tool. Duplicate keys are reported.
- **Presets:** `toml/taplo-compat` (`collapse: always`, `max_blank_lines: 2`, `align_comments: true`).
- **Options:** `collapse`, `trailing_commas`, `align_comments`, `max_blank_lines`, `bracket_spacing`, `array_bracket_spacing`.
- **Differences:**
  - Taplo keeps CRLF inside multi-line strings. Sudsy writes LF.
  - Taplo expands arrays inside long inline tables. Sudsy breaks only what does not fit.
  - Taplo rejects some valid files. Sudsy formats them.
- **Reproduce (needs a source checkout):** `scripts/parity.sh format taplo`.

## HTML, Vue, Svelte, Angular

- One parser serves all four. Config sections: `html`, `vue`, `svelte`, `angular`.
- **Formatter:** Prettier 3.9.10 (Svelte: with prettier-plugin-svelte 3.5.2).

- **Corpora:** real-world files and Prettier's own tests for HTML, Vue and Svelte; the prettier-plugin-svelte samples; Angular components and Prettier's Angular tests (`--parser angular`).
- **Linters:** html-validate, htmlhint, html-eslint, eslint-plugin-vue, eslint-plugin-svelte. Angular: a few angular-eslint rules, no coverage run.
- **Presets:** `html/html-validate-recommended`, `html/html-validate-standard`, `html/htmlhint`, `html/html-eslint-recommended`, `html/eslint-plugin-vue-recommended`, `html/eslint-plugin-svelte-recommended`.
- **Options:** `whitespace_sensitivity`, `bracket_same_line`, `single_attribute_per_line`, `embedded_language_formatting`. Vue and Svelte add `indent_script_and_style`. Default `line_width` is 80.
- **Differences:** Vue and Svelte differ most. The named gaps are in the source checkout.
- **Reproduce (needs a source checkout):** `scripts/parity.sh format prettier-html`, `scripts/parity.sh format prettier-angular`, `scripts/parity.sh coverage html-validate`.

## PowerShell

- **Formatter:** Invoke-Formatter (PSScriptAnalyzer 1.25.0). Trailing blanks are ignored in the comparison.
- **Linter:** PSScriptAnalyzer.
- **Presets:** `powershell/psscriptanalyzer`.
- **Options:** `max_blank_lines` (default 10).
- **Differences:**
  - Invoke-Formatter outdents a script block after a backtick continuation. Sudsy keeps its indentation.
  - The module knows types of scalar variables. Sudsy has no types, so some null-comparison findings differ.
- **Reproduce (needs a source checkout):** `scripts/parity.sh format invoke-formatter`, `scripts/parity.sh coverage psscriptanalyzer`. Needs PowerShell.

## PHP

- Config section: `php`.
- **Formatter:** PHP-CS-Fixer 3.95.31 with `@PER-CS`. Corpus: Laravel, Symfony, WordPress, Composer.
- **Linter:** PHPStan mapping exists. Sudsy does no type analysis.
- **Presets:** `php/phpstan`.
- **Options:** `trailing_commas`, `declaration_brace`. Default `line_width` is 120.
- **Differences:**
  - Files with inline text (`?>`): PHP-CS-Fixer formats the PHP islands. Sudsy keeps the file as written.
  - One `use` per trait is not split.
  - `else if` to `elseif`, modifier order and import order are not rewritten.
- **Reproduce (needs a source checkout):** `scripts/parity.sh format php-cs-fixer`, `scripts/parity.sh coverage phpstan`.

## Markdown

- **Formatter:** Prettier 3.9.10. Corpus: the repository docs and the dotnet coding guidelines. Small corpus.
- **Linter:** markdownlint.
- **Presets:** `markdown/prettier-compat`, `markdown/markdownlint`.
- **Options:** `embedded_language_formatting` (formats fenced code with that language's formatter).
- **Differences:**
  - Trailing blanks inside code blocks stay. Prettier removes them.
  - A block with a tab is kept as written.
  - A table of contents with unusual list indent is kept as written.
- **Reproduce (needs a source checkout):** `scripts/parity.sh format prettier`, `scripts/parity.sh coverage markdownlint`.

## Java

- **Formatter:** google-java-format 2.0.0, run with `--skip-javadoc-formatting --skip-removing-unused-imports --skip-reflowing-long-strings`. Corpus: Guava, Gson, Error Prone, Commons Lang.
- **Linter:** Checkstyle.
- **Presets:** `java/google`.
- **Options:** `member_chain_*`, `binary_*`, `assignment_*` and `conditional_*` layout options. Default `line_width` is 100.
- **Differences:**
  - Long line comments are not re-wrapped.
  - A method chain inside a ternary is indented differently.
  - `@A()` keeps its parentheses.
- **Reproduce (needs a source checkout):** `scripts/parity.sh format google-java-format`, `scripts/parity.sh coverage checkstyle`.

## Shell

- **Formatter:** shfmt 3.14.1. Corpus: install scripts of dotnet, nvm, rustup, docker, Homebrew and others.
- **Linter:** ShellCheck 0.11.0.
- **Presets:** `shell/shellcheck`, `shell/shfmt`.
- **Options:** `max_blank_lines`, `comment_column`, `heredoc_indent`, `case_last_terminator`.
- **Differences:**
  - Backticks stay. shfmt rewrites them to `$(...)`.
  - Without the `shell/shfmt` preset a last `case` arm without `;;` stays, tabs in `<<-` heredocs are kept and trailing comments keep their blanks.
- **Reproduce (needs a source checkout):** `scripts/parity.sh format shfmt`, `scripts/parity.sh coverage shellcheck`.

## SQL

- Dialects: `ansi`, `postgres`, `mysql`, `tsql` (option `dialect`).
- **Formatter:** sqlfluff 4.5.0 (`fix --rules layout,capitalisation`).
- **Linter:** sqlfluff.
- **Presets:** `sql/sqlfluff`.
- **Options:** `dialect`, `comma_position`, `keyword_case`, `identifier_case`, `function_case`, `literal_case`, `type_case`. Default `line_width` is 80.
- **Differences:**
  - Files the parser does not accept are refused.
  - Layout of some constructs is not written: expressions, joins, CASE, CTEs, comments. Those parts stay as written.
- **Reproduce (needs a source checkout):** `scripts/parity.sh format sqlfluff`, `scripts/parity.sh coverage sqlfluff`.

## C#

- Needs the .NET 10 runtime. Config section: `csharp`.
- **Formatter:** CSharpier 1.3.0. Corpus: aspnetcore, Roslyn and runtime; the files CSharpier leaves alone are not compared.
- **Linter:** StyleCop and Roslynator rules. StyleCop (1.2.0-beta.556, run as an analyzer driver over the corpus) is covered at 23.5% of its findings (100,884 of 428,560): `SA1101`, `SA1600`, `SA1611`, `SA1200` are not rules yet. Roslynator has no findings run.
- **Presets:** `csharp/csharpier-compat`, `csharp/stylecop`, `csharp/dotnet-runtime`, `csharp/google`, `csharp/unity`.
- **Options:** `brace_newline`, `brace_same_line`, `trailing_commas`, `sort_usings`, `member_chain_*`, `preprocessor_symbols`, `derive_symbol_sets`, `blank_lines_between_members`, `empty_braces`. Default `line_width` is 120.
- **Differences:**
  - Lists that differ by a comma, mostly initializers after wrapped arguments.
  - Comment placement and text next to `#if` directives .
  - Wrapped arguments and member chains .
  - CSharpier formats interpolated string holes. Sudsy does not.
- **Reproduce (needs a source checkout):** `scripts/parity.sh format csharpier`, `scripts/parity.sh format dotnet-format`, `scripts/parity.sh coverage stylecop`. All need the .NET SDK.

## Kotlin

- **Formatter:** ktlint 1.8.0 (`ktlint_official`). Corpus: coroutines, ktor, nowinandroid, ktlint.
- **Linter:** ktlint.
- **Presets:** `kotlin/ktlint`.
- **Options:** `trailing_commas`, `function_signature_wrap_min_parameters`, `class_signature_wrap_min_parameters`, `member_chain_style`. Default `line_width` is 140.
- **Differences:**
  - ktlint rewrites are not done: `: Unit`, `;`, modifier order, `//x` to `// x`.
  - Operator chains follow the source breaks.
  - Values ktlint moves below `=` differ in some cases.
- **Reproduce (needs a source checkout):** `scripts/parity.sh format ktlint`, `scripts/parity.sh coverage ktlint`.

## YAML

- Config section: `yaml`.
- **Formatter:** Prettier 3.9.10. Corpus: 849 YAML files of real projects (awesome-compose, the Kubernetes website, GitHub starter workflows, actionlint); 803 identical (94.6%). The yaml-test-suite valid cases parse, round-trip and keep their data model.
- **Linter:** yamllint.
- **Presets:** `yaml/prettier-compat`, `yaml/yamllint`.
- **Options:** `quote_style`, `sequence_indent`, `document_start`, `document_end`, `max_blank_lines`, `comment_spacing`, `bracket_spacing`. Default `line_width` is 80.
- **Differences:**
  - Long scalars and comments are not re-wrapped.
  - A broken flow sequence has no trailing comma; Prettier writes one.
  - A comment before a dedented item takes the next line's column; Prettier keeps its own.
- **Reproduce (needs a source checkout):** `scripts/parity.sh format prettier`, `scripts/parity.sh coverage yamllint`.

## JSON

- Covers JSONC. Config section: `json`.
- **Formatter:** Prettier 3.9.10 on 3,212 JSON files (SchemaStore, VS Code extension samples): 3,184 identical (99.1%). Biome on fixtures.
- **Linter:** eslint-plugin-jsonc recommended: on the corpus only `no-useless-escape` fires (14 findings) and Sudsy does not check string escapes yet, so 0.0%.
- **Presets:** `json/prettier-compat`, `json/biome`, `json/dprint`, `json/eslint-plugin-jsonc-recommended`.
- **Options:** `collapse` (`preserve`, `always`, `never`, `opening_break`), `trailing_commas` (JSONC), `dialect` (`auto`, `json`, `jsonc`), `bracket_spacing`. Default `line_width` is 80.
- **Differences:**
  - An array of numbers that does not fit gets one number per line; Prettier fills lines.
  - An array of arrays that fits stays on one line; Prettier breaks the outer array.
  - A `.json` file with a trailing comma is refused (the `jsonc` dialect allows it).
- **Reproduce (needs a source checkout):** `scripts/parity.sh format prettier`, `scripts/parity.sh format biome`, `scripts/parity.sh coverage eslint-jsonc`.

## CSS, SCSS, Less

- Config section: `css` (also `scss`, `less`).
- **Formatter:** Prettier 3.9.10 (`--no-config`). Corpus: real-world projects and Prettier's own CSS, SCSS and Less tests; the conformance corpus (Bootstrap, Bulma and others).
- **Linter:** stylelint standard. Coverage of stylelint-config-standard-scss: 20.6% (599 of 2,904; the `scss/` plugin rules have no Sudsy rule yet). Of stylelint-less: 78.0% (7,834 of 10,039).
- **Presets:** `css/stylelint-recommended`, `css/stylelint-standard`.
- **Options:** `single_quote`, `max_blank_lines`. Default `line_width` is 80.
- **Differences:**
  - Some SCSS maps and `@include` with comments stay as written.
  - Less mixin calls with `{ }` arguments stay as written.
  - A comment inside a selector or between property and value keeps the statement as written.
- **Reproduce (needs a source checkout):** `scripts/parity.sh format prettier-css`, `scripts/parity.sh coverage stylelint`, `scripts/parity.sh coverage stylelint-scss`, `scripts/parity.sh coverage stylelint-less`.

## XML

- Covers `.csproj`, `.props`, `.config`.
- **Formatter:** CSharpier XML on aspnetcore project files and fixtures.
- **Linter:** none.
- **Presets:** `xml/csharpier-compat`.
- **Options:** `max_blank_lines`. Default `line_width` is 100.
- **Differences:** line width is counted differently in some `.resx` files.
- **Reproduce (needs a source checkout):** `scripts/parity.sh format csharpier-xml`.

## Ruby

- The formatter keeps files as written.
- **Linter:** RuboCop Lint department. Other departments are not covered.
- **Presets:** `ruby/rubocop`.
- **Reproduce (needs a source checkout):** `scripts/parity.sh coverage rubocop`.

## Dockerfile

- The formatter fixes keyword case and blank lines. No reference formatter exists: the format column is the stable share.
- **Linter:** hadolint.
- **Presets:** `dockerfile/hadolint`.
- **Options:** `max_blank_lines`.
- **Differences:** a variable as `USER` is accepted whether or not an `ARG` defines it. hadolint reports it.
- **Reproduce (needs a source checkout):** `scripts/parity.sh coverage hadolint` (lint), `scripts/parity.sh stability dockerfile` (stable).

## Makefile

- The formatter normalizes assignment spacing and blank lines. No reference formatter exists: the format column is the stable share.
- **Linter:** checkmake.
- **Presets:** `makefile/checkmake`.
- **Options:** `space_around_assignment`, `space_before_colon`, `max_blank_lines` (default 2).
- **Differences:** `A+=b`, `A?=b` and `A!=b` without a blank before the operator are left as written.
- **Reproduce (needs a source checkout):** `scripts/parity.sh coverage checkmake` (lint), `scripts/parity.sh stability makefile` (stable).

## GitHub Actions

- Files use the YAML formatter. `run:` scripts are checked as Shell.
- **Linter:** actionlint. Only errors and `script-injection` are on by default.
- **Presets:** `github-actions/actionlint`.
- **Options:** the YAML options. Config section: `github-actions`, on top of `yaml`.
- **Reproduce (needs a source checkout):** `scripts/parity.sh coverage actionlint` (lint), `scripts/parity.sh stability github-actions` (stable).

## C / C++

- Files are kept as written. There are no lint rules.
- **Parser:** a lexer with a token-level structure. There is no full grammar.
- **Not done:** a clang-format compatible layout.

## INI, EditorConfig, .gitignore

- Small formatters and a few rules. No reference tool, so no comparison: the format column is the stable share.
- **Options:** `max_blank_lines`. INI adds `space_around_equals`.
- **Rules:** invalid or duplicate entries, for example unknown EditorConfig properties and ignored `.gitignore` patterns.

## Language packs

These languages are small downloads, fetched with `sudsy languages add`: editorconfig, gitignore, graphql, hcl, ini, markdown, powershell, protobuf, shell, toml, xml, dockerfile and makefile. Every other language is built in. Until you fetch a pack, its files are skipped with a warning. See [configuration](configuration.md#language-packs).

## Reproduce (needs a source checkout)

Each comparison runs from a clone of the repository. `scripts/parity.sh format <tool>` compares formatting. `scripts/parity.sh coverage <tool>` measures lint findings. Each run downloads the tool and the corpus. The complete list of differences per tool is in `tests/parity/<tool>/differences.yaml` and in the generated sections of the language pages under [ai/formatting/](ai/formatting/js.md). Method: [ai/development/parity.md](ai/development/parity.md).

More: [format options in detail](ai/formatting/presets.md), [lint rule coverage](ai/parity/README.md), [how comparisons run](ai/development/parity.md).
