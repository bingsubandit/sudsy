# Parity catalog

What the tools Sudsy aims to replace do, and where Sudsy stands. One file per tool (`docs/ai/parity/<tool>.yaml`); `tool`, `version` (upstream; the parity runner installs it), `languages`, `sources`, `scope` and `rule-url` head each file. `rule-url` is the link of one upstream rule's own page, a template over `{id}`, `{id-lower}`, `{id-flat}` (lower case, no dashes), `{group}` (the part of the id before the first `/` or `-`) and `{name}` (the part after it), or `none` for a tool without per-rule pages (the site then links the tool's home page); `rule-url-strip: <prefix>` removes a prefix from the id first, and `rule-urls:` maps single ids to their own URL where the template does not fit. Only rule entries get a link, never option, formatter or config entries. `node site/scripts/check-urls.mjs` (`scripts/site.sh check-links`, nightly) fetches every resolved URL and its `#anchor`. A test (`crates/languages/tests/parity.rs`) fails when a `rule` or `option` status names something that does not exist; the counts below are generated (`cargo xtask codegen`). The differential runner [`scripts/parity.sh`](../development/parity.md) compares a tool's output with `sudsy lint` on fixtures.

`Ready` of the README table: the primary tool of each leg at 85%. A new language's leg with no reference tool has its own bars, `stable` 100% and agreement 80% (format), checklist 85% and precision 90% (lint), from the `evidence` folder of its `xtask/full/support.yaml` row ([../development/no-reference-tool.md](../development/no-reference-tool.md#6-ready-numbers)); the six oldest such legs are `exempt`.

## Format
The catalog of a lint tool lists every rule of the pinned version (`scripts/parity_inventory.py <tool> --write` appends the missing ones as `planned`; a deprecated rule as `wontfix`, a ruff preview rule as `deferred`), so the counts below are inventory sizes, not progress: how much of a tool's output Sudsy reproduces is the Coverage share column (`scripts/parity.sh coverage <tool>`, [../development/parity.md](../development/parity.md#coverage-mode); `-` where the tool has no corpus yet) and the Target column beside it (the same share on the target number: the findings of out-of-scope `wontfix` entries leave it, the findings of `formatter` entries count as missing until a proof run; `-` until the tool's `target:` ratchet is set; [../design/parity-target.md](../design/parity-target.md)). One line per upstream rule or option under `entries:`, `<id>: <status>` or `<id>: <status> | <gap>`; the id of an option is `line-length.max`, so tools with many sub-options weigh more in the counts. `<gap>` says what is missing and makes a `rule`, `option`, `formatter` or `config` entry `partial`; for `planned` and `deferred` it is the reason. Ids that are only deferred are grouped by reason under a top-level `deferred:` (`- gap: "<reason>"`, `ids: [DL3049, DL3050]`). `languages::catalog::parse` expands both forms; coverage follows from the status, so it is not written.

| Status | Meaning | Coverage |
|---|---|---|
| `rule <id>[, <id>...]` | A Sudsy rule does it, or all of the listed ones when one upstream rule is split over ids (`syntax-error` is the parser's diagnostic); the rule's `sudsy rules` page and the table in [rules/builtin.md](../rules/builtin.md) list the entry as its equivalent. | covered, partial with a gap |
| `option <language>.format.<name>` | A formatter option does it. | covered, partial with a gap |
| `formatter` | Fixed formatter behavior, no option. | covered, partial with a gap |
| `config <text>` | Configuration outside rules and options (overrides, ignore files, presets, suppression comments). | covered, partial with a gap |
| `sdk <id>` | A .NET SDK diagnostic (IDE, CA, CS) does it and Sudsy has no rule for it (StyleCop ids with a rule are `rule`: [../design/stylecop-rules.md](../design/stylecop-rules.md)). | sdk |
| `planned` | Accepted for implementation. | missing |
| `deferred` | Wanted eventually, deliberately not scheduled. | deferred |
| `wontfix <reason>` | Out of scope. | wontfix |

The clang-format run does not compare line wrapping (the `wrap_compare` `note` of its entry in `scripts/parity_tools.json`): its format share counts files whose tokens and non-wrap gaps equal clang-format's, and the byte-identical share is printed beside it.

## Tools and corpora on a fresh host
`scripts/parity.sh` installs a tool from its `install` key in `scripts/parity_tools.json` (npm, pip, a release file, a PowerShell module with `psmodule`) and runs the `fetch` scripts of its corpus (`tests/conformance/fetch-<language>-corpus.sh`; the corpora are gitignored, so a new checkout fetches them first: a run on an empty corpus prints `0 of 0 findings`, which is a missing corpus, not a result). Two things come from outside the runner: PowerShell (`pwsh` 7.4 or later on `PATH`; on Linux the tarball `powershell-<version>-linux-<x64|arm64>.tar.gz` of the PowerShell GitHub release, unpacked anywhere) for `invoke-formatter` and `psscriptanalyzer`, and the .NET SDK for the C# tools. PHP is not needed: `php-cs-fixer` and `phpstan` unpack a static PHP of dl.static-php.dev into their prefix. The nightly matrix runs the same commands. Hosts used: registry.npmjs.org, pypi.org, github.com releases, dl.static-php.dev, www.powershellgallery.com, api.nuget.org (StyleCop.Analyzers for the fixtures and the driver), repo.packagist.org and api.github.com (the PHP corpus's Composer packages).

## Counts
<!-- generated:counts -->
| Tool | Target | Measured on | Entries | Covered | Partial | Missing | SDK | Wontfix | Deferred | Planned |
|---|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|
| actionlint | 87.0% | `cbdd3e85` | 43 | 5 | 30 | 0 | 0 | 0 | 8 | 0 |
| angular-eslint | 99.8% | `b314ffb4` | 44 | 10 | 4 | 20 | 0 | 0 | 10 | 20 |
| biome | - | - | 34 | 13 | 6 | 2 | 0 | 13 | 0 | 2 |
| buf | 89.7% | `2a570ae8` | 47 | 22 | 3 | 0 | 0 | 0 | 22 | 0 |
| checkmake | 100.0% | `7514c1f3` | 5 | 4 | 1 | 0 | 0 | 0 | 0 | 0 |
| checkstyle | 88.6% | `f29ba2f0` | 132 | 37 | 24 | 65 | 0 | 6 | 0 | 65 |
| clang-format | - | - | 2 | 2 | 0 | 0 | 0 | 0 | 0 | 0 |
| cmake-lint | 97.2% | `92bc171a` | 35 | 8 | 0 | 24 | 0 | 0 | 3 | 24 |
| csharpier | - | - | 16 | 11 | 5 | 0 | 0 | 0 | 0 | 0 |
| dockerfmt | - | - | 7 | 3 | 0 | 0 | 0 | 3 | 1 | 0 |
| dotnet-format-editorconfig | - | - | 56 | 50 | 3 | 0 | 1 | 1 | 1 | 0 |
| eslint | 100.0% | `7514c1f3` | 292 | 95 | 22 | 135 | 0 | 19 | 21 | 135 |
| eslint-plugin-jsonc | 100.0% | `7514c1f3` | 44 | 35 | 7 | 0 | 0 | 2 | 0 | 0 |
| eslint-plugin-svelte | 100.0% | `7514c1f3` | 85 | 19 | 9 | 54 | 0 | 3 | 0 | 54 |
| eslint-plugin-vue | 88.8% | `7514c1f3` | 256 | 45 | 58 | 149 | 0 | 4 | 0 | 149 |
| gersemi | - | - | 6 | 2 | 0 | 0 | 0 | 0 | 4 | 0 |
| google-java-format | - | - | 14 | 2 | 5 | 1 | 0 | 6 | 0 | 1 |
| graphql-eslint | 88.1% | `7514c1f3` | 65 | 4 | 6 | 30 | 0 | 0 | 25 | 30 |
| hadolint | 91.2% | `7514c1f3` | 71 | 38 | 13 | 0 | 0 | 1 | 19 | 0 |
| html-eslint | 88.1% | `916059a8` | 66 | 15 | 18 | 33 | 0 | 0 | 0 | 33 |
| html-validate | 92.5% | `2607e39a` | 97 | 34 | 40 | 20 | 0 | 1 | 2 | 20 |
| htmlhint | 91.6% | `0c8a3dcc` | 44 | 10 | 7 | 27 | 0 | 0 | 0 | 27 |
| ktlint | 92.3% | `bede0bee` | 100 | 53 | 11 | 36 | 0 | 0 | 0 | 36 |
| markdownlint | 99.0% | `7514c1f3` | 53 | 23 | 19 | 0 | 0 | 0 | 11 | 0 |
| php-cs-fixer | - | - | 1 | 0 | 1 | 0 | 0 | 0 | 0 | 0 |
| phpstan | 36.3% | `062a8f18` | 19 | 0 | 7 | 10 | 0 | 2 | 0 | 10 |
| prettier | - | - | 59 | 38 | 9 | 2 | 0 | 3 | 7 | 2 |
| psscriptanalyzer | 87.1% | `7514c1f3` | 75 | 12 | 14 | 1 | 0 | 0 | 48 | 1 |
| revive | 97.5% | `1d6de91f` | 105 | 12 | 4 | 7 | 0 | 0 | 82 | 7 |
| roslynator | 0.4% | `66718389` | 340 | 28 | 7 | 286 | 18 | 1 | 0 | 286 |
| rubocop | 96.2% | `7514c1f3` | 160 | 27 | 5 | 69 | 0 | 0 | 59 | 69 |
| ruff | 94.6% | `7514c1f3` | 980 | 134 | 1 | 687 | 0 | 18 | 140 | 687 |
| shellcheck | 86.6% | `7514c1f3` | 520 | 19 | 22 | 0 | 0 | 0 | 479 | 0 |
| shfmt | - | - | 9 | 1 | 1 | 0 | 0 | 3 | 4 | 0 |
| sqlfluff | 90.2% | `bede0bee` | 97 | 47 | 0 | 31 | 0 | 0 | 19 | 31 |
| stylecop | 87.3% | `e98263ba` | 197 | 126 | 10 | 1 | 39 | 21 | 0 | 1 |
| stylelint | 94.8% | `7514c1f3` | 150 | 90 | 11 | 45 | 0 | 0 | 4 | 45 |
| stylelint-less | 85.4% | `7514c1f3` | 6 | 0 | 0 | 5 | 0 | 0 | 1 | 5 |
| stylelint-scss | 89.9% | `7514c1f3` | 71 | 16 | 2 | 52 | 0 | 0 | 1 | 52 |
| taplo | - | - | 23 | 11 | 7 | 0 | 0 | 0 | 5 | 0 |
| terraform | - | - | 2 | 1 | 1 | 0 | 0 | 0 | 0 | 0 |
| tflint | 93.0% | `7514c1f3` | 20 | 8 | 4 | 0 | 0 | 0 | 8 | 0 |
| typescript-eslint | 100.0% | `7514c1f3` | 136 | 10 | 16 | 34 | 0 | 9 | 67 | 34 |
| yamllint | 98.5% | `dc16e446` | 66 | 61 | 4 | 1 | 0 | 0 | 0 | 1 |
<!-- /generated -->

## Lint coverage by function
The rules with most findings that Sudsy does not report, per reference tool, on its real-world corpus with the Sudsy preset that mirrors the tool's default or recommended set (`scripts/parity.sh coverage <tool>`, [../development/parity.md](../development/parity.md#coverage-mode)). The number is the Target column above ([../design/parity-target.md](../design/parity-target.md)); the usability bar is 85%. Sudsy's own defaults keep the bug-only policy ([../config/sudsy-yaml.md](../config/sudsy-yaml.md#zero-config)), so a preset is needed to reproduce a tool's set. Top missing rules: the five with most findings Sudsy does not report (rules with no Sudsy rule yet, or `planned`).

| Language | Tool | Preset | Top 5 missing rules (findings) |
|---|---|---|---|
| Python | ruff | `python/ruff` | `RUF100` 83, `RUF028` 8, `PYI034` 7, `UP028` 5, `PYI036` 5 |
| Ruby | rubocop | `ruby/rubocop` | `Lint/EmptyFile` 7, `Lint/RedundantSplatExpansion` 6, `Lint/NonLocalExitFromIterator` 6, `Lint/DeprecatedOpenSSLConstant` 6, `Lint/AmbiguousRegexpLiteral` 6 |
| JavaScript | eslint | `js/eslint-recommended` | - |
| TypeScript | typescript-eslint | `js/typescript-eslint-recommended` | - |
| Vue | eslint-plugin-vue | `html/eslint-plugin-vue-recommended` | `vue/attributes-order` 81, `vue/html-self-closing` 71, `vue/no-v-html` 25, `vue/no-template-shadow` 3, `vue/return-in-computed-property` 1 |
| Svelte | eslint-plugin-svelte | `html/eslint-plugin-svelte-recommended` | - |
| YAML | yamllint | `yaml/yamllint` | `comments` 325, `indentation` 160, `syntax` 136, `braces` 57, `document-start` 52 |
| CSS | stylelint | `css/stylelint-standard` | `media-feature-range-notation` 415, `value-keyword-case` 127, `no-descending-specificity` 74 |
| HTML | html-validate | `html/html-validate-recommended` | `wcag/h63` 48, `wcag/h30` 23, `element-permitted-content` 23, `parser-error` 21, `element-required-content` 20 |
| HTML | htmlhint | `html/htmlhint` | `spec-char-escape` 1087, `tag-pair` 105, `doctype-first` 36, `src-not-empty` 6, `title-require` 1 |
| HTML | html-eslint | `html/html-eslint-recommended` | `indent` 16182, `element-newline` 2175, `attrs-newline` 1158, `require-closing-tags` 146, `no-extra-spacing-tags` 132 |
| Markdown | markdownlint | `markdown/markdownlint` | `MD010` 309, `MD037` 6, `MD041` 4, `MD051` 2, `MD028` 1 |
| Dockerfile | hadolint | `dockerfile/hadolint` | `DL3066` 30, `SC2086` 28, `SC2155` 24, `SC1065` 14, `SC2046` 12 |
| Shell | shellcheck | `shell/shellcheck` | `SC2140` 22, `SC2034` 14, `SC1091` 12, `SC2154` 11, `SC2206` 9 |
| GitHub Actions | actionlint | `github-actions/actionlint` | `syntax-check` 235, `expression/types` 155, `events` 115, `action` 41, `glob` 22 |
| Makefile | checkmake | `makefile/checkmake` | `uniquetargets` 88, `phonydeclared` 59, `minphony` 5 |
| Protobuf | buf | `protobuf/buf-standard` | `RPC_REQUEST_RESPONSE_UNIQUE` 117, `PACKAGE_DIRECTORY_MATCH` 98, `RPC_RESPONSE_STANDARD_NAME` 64, `FIELD_NOT_REQUIRED` 39, `RPC_REQUEST_STANDARD_NAME` 9 |
| PowerShell | psscriptanalyzer | `powershell/psscriptanalyzer` | `PSUseShouldProcessForStateChangingFunctions` 140, `PSUseDeclaredVarsMoreThanAssignments` 130, `PSUseCmdletCorrectly` 123, `PSReviewUnusedParameter` 123, `PSUseSingularNouns` 59 |
| Kotlin | ktlint | `kotlin/ktlint` | `function-signature` 30800, `argument-list-wrapping` 24113, `multiline-expression-wrapping` 15169, `class-signature` 4591, `trailing-comma-on-call-site` 4162 |
| HCL | tflint | `hcl/tflint-recommended` | `terraform_required_providers` 37, `terraform_required_version` 20, `terraform_unused_declarations` 2 |
| Java | checkstyle | `java/google` | `MissingJavadocMethod` 18283, `MethodName` 7887, `JavadocParagraph` 5301, `SummaryJavadoc` 1527, `TodoFormat` 1498 |
| PHP | phpstan | `php/phpstan` | `method.notFound` 384, `class.notFound` 381, `new.static` 317, `function.notFound` 140, `staticMethod.notFound` 90 |
| SQL | sqlfluff | `sql/sqlfluff` | `PRS` 380, `CP05` 227, `CP02` 187, `RF05` 131, `CV06` 126 |

Not measured:

| Tool | Reason |
|---|---|
| clang-tidy | no catalog, and Sudsy has no C or C++ lint rules; a share would be 0 by construction |
| detekt, PMD, PHPCS | not catalogued |
| eslint-plugin-jsonc | the JSON corpus is a parser test suite (JSONTestSuite), no real-world files |
| stylelint-scss, stylelint-less | plugin rules without a recommended set of their own; need SCSS and Less configs and parsers |
| graphql-eslint, biome, taplo, terraform | no coverage entry yet (fixtures only) |

Sets and corpora of each run: [../development/parity.md](../development/parity.md#coverage-mode). Findings of `formatter` entries are left out; a file the tool cannot read is left out and counted by the run. The layout rules that are `planned` in a catalog (html-eslint `indent`, ktlint `function-signature`) count as missing until the catalog says `formatter` ([../roadmap.md](../roadmap.md#small-tasks)).

## Migration
`sudsy init --from <tool>` ([config/cli.md](../config/cli.md#init---from)) turns a tool's config into a `sudsy.yaml` with this catalog as the source of truth: the catalog files are embedded in the binary, the reason printed for a setting that cannot be migrated is the entry's `gap` (or its status), and the C# `.editorconfig` entries that switch off covered SA rules come from the `covered` StyleCop entries. The option and rule values are mapped in code (`crates/cli/src/init/`); each mapping names its catalog entry and debug builds fail when the entry points elsewhere. A new `rule:` or `option:` status therefore changes what `init` writes for the settings that use it; the fixtures under `tests/fixtures/init/` show the result.

## Equivalent ids of rules
`sudsy rules <id>` and the table in [rules/builtin.md](../rules/builtin.md) list as `Equivalent` every `covered` or `partial` entry whose status is `rule <id>` in a catalog for one of the rule's languages (`languages::catalog::equivalents`). Rule descriptions do not name other tools; add or fix the entry here instead.
