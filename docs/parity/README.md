# Parity catalog

What the tools Sudsy aims to replace do, and where Sudsy stands. One file per tool (`docs/parity/<tool>.yaml`); `tool`, `version` (upstream; the parity runner installs it), `languages`, `sources` and `scope` head each file. A test (`crates/languages/tests/parity.rs`) fails when a `rule` or `option` status names something that does not exist; the counts below are generated (`cargo xtask codegen`). The differential runner [`scripts/parity.sh`](../development/parity.md) compares a tool's output with `sudsy lint` on fixtures.

## Format
The catalog of a lint tool lists every rule of the pinned version (`scripts/parity_inventory.py <tool> --write` appends the missing ones as `planned`; a deprecated rule as `wontfix`, a ruff preview rule as `deferred`), so the counts below are inventory sizes, not progress: how much of a tool's output Sudsy reproduces is the Coverage share column (`scripts/parity.sh coverage <tool>`, [../development/parity.md](../development/parity.md#coverage-mode); `-` where the tool has no corpus yet). One line per upstream rule or option under `entries:`, `<id>: <status>` or `<id>: <status> | <gap>`; the id of an option is `line-length.max`, so tools with many sub-options weigh more in the counts. `<gap>` says what is missing and makes a `rule`, `option`, `formatter` or `config` entry `partial`; for `planned` and `deferred` it is the reason. Ids that are only deferred are grouped by reason under a top-level `deferred:` (`- gap: "<reason>"`, `ids: [DL3049, DL3050]`). `languages::catalog::parse` expands both forms; coverage follows from the status, so it is not written.

| Status | Meaning | Coverage |
|---|---|---|
| `rule <id>` | A Sudsy rule does it (`syntax-error` is the parser's diagnostic); the rule's `sudsy rules` page and the table in [rules/builtin.md](../rules/builtin.md) list the entry as its equivalent. | covered, partial with a gap |
| `option <language>.format.<name>` | A formatter option does it. | covered, partial with a gap |
| `formatter` | Fixed formatter behavior, no option. | covered, partial with a gap |
| `config <text>` | Configuration outside rules and options (overrides, ignore files, presets, suppression comments). | covered, partial with a gap |
| `sdk <id>` | A .NET SDK diagnostic (IDE, CA, CS) does it; Sudsy never reimplements these. | sdk |
| `planned` | Accepted for implementation. | missing |
| `deferred` | Wanted eventually, deliberately not scheduled. | deferred |
| `wontfix <reason>` | Out of scope. | wontfix |

## Counts
<!-- generated:counts -->
| Tool | Coverage share | Entries | Covered | Partial | Missing | SDK | Wontfix | Deferred | Planned |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| actionlint | - | 26 | 5 | 4 | 0 | 0 | 0 | 17 | 0 |
| angular-eslint | - | 37 | 3 | 2 | 22 | 0 | 0 | 10 | 22 |
| biome | - | 34 | 13 | 6 | 2 | 0 | 13 | 0 | 2 |
| buf | - | 47 | 20 | 3 | 0 | 0 | 0 | 24 | 0 |
| checkmake | - | 5 | 4 | 1 | 0 | 0 | 0 | 0 | 0 |
| checkstyle | - | 126 | 36 | 12 | 72 | 0 | 6 | 0 | 72 |
| csharpier | - | 16 | 11 | 5 | 0 | 0 | 0 | 0 | 0 |
| dotnet-format-editorconfig | - | 56 | 50 | 3 | 0 | 1 | 2 | 0 | 0 |
| eslint | 100.0% | 292 | 92 | 19 | 128 | 0 | 32 | 21 | 128 |
| eslint-plugin-jsonc | - | 44 | 36 | 6 | 0 | 0 | 2 | 0 | 0 |
| eslint-plugin-svelte | - | 85 | 18 | 8 | 18 | 0 | 3 | 38 | 18 |
| eslint-plugin-vue | - | 256 | 43 | 16 | 176 | 0 | 4 | 17 | 176 |
| google-java-format | - | 14 | 2 | 5 | 1 | 0 | 6 | 0 | 1 |
| graphql-eslint | - | 65 | 4 | 6 | 30 | 0 | 0 | 25 | 30 |
| hadolint | - | 71 | 38 | 10 | 0 | 0 | 1 | 22 | 0 |
| html-eslint | - | 66 | 14 | 11 | 41 | 0 | 0 | 0 | 41 |
| html-validate | - | 97 | 34 | 30 | 26 | 0 | 1 | 6 | 26 |
| htmlhint | - | 44 | 10 | 6 | 28 | 0 | 0 | 0 | 28 |
| ktlint | - | 97 | 34 | 7 | 56 | 0 | 0 | 0 | 56 |
| markdownlint | - | 53 | 25 | 16 | 0 | 0 | 0 | 12 | 0 |
| phpstan | - | 1 | 0 | 1 | 0 | 0 | 0 | 0 | 0 |
| prettier | - | 59 | 38 | 9 | 2 | 0 | 9 | 1 | 2 |
| psscriptanalyzer | - | 75 | 12 | 8 | 0 | 0 | 1 | 54 | 0 |
| roslynator | - | 340 | 23 | 7 | 289 | 18 | 1 | 2 | 289 |
| ruff | 6.7% | 980 | 88 | 0 | 735 | 0 | 19 | 138 | 735 |
| shellcheck | - | 520 | 17 | 8 | 0 | 0 | 0 | 495 | 0 |
| shfmt | - | 9 | 1 | 1 | 0 | 0 | 3 | 4 | 0 |
| sqlfluff | - | 97 | 35 | 0 | 43 | 0 | 4 | 15 | 43 |
| stylecop | - | 197 | 120 | 10 | 1 | 45 | 21 | 0 | 1 |
| stylelint | - | 150 | 83 | 9 | 54 | 0 | 0 | 4 | 54 |
| stylelint-less | - | 6 | 0 | 0 | 5 | 0 | 0 | 1 | 5 |
| stylelint-scss | - | 71 | 1 | 2 | 67 | 0 | 0 | 1 | 67 |
| taplo | - | 23 | 11 | 7 | 0 | 0 | 0 | 5 | 0 |
| terraform | - | 2 | 1 | 1 | 0 | 0 | 0 | 0 | 0 |
| tflint | - | 20 | 8 | 4 | 0 | 0 | 0 | 8 | 0 |
| typescript-eslint | 100.0% | 136 | 10 | 16 | 34 | 0 | 9 | 67 | 34 |
| yamllint | - | 66 | 62 | 3 | 1 | 0 | 0 | 0 | 1 |
<!-- /generated -->

## Migration
`sudsy init --from <tool>` ([config/cli.md](../config/cli.md#init---from)) turns a tool's config into a `sudsy.yaml` with this catalog as the source of truth: the catalog files are embedded in the binary, the reason printed for a setting that cannot be migrated is the entry's `gap` (or its status), and the C# `.editorconfig` entries that switch off covered SA rules come from the `covered` StyleCop entries. The option and rule values are mapped in code (`crates/cli/src/init/`); each mapping names its catalog entry and debug builds fail when the entry points elsewhere. A new `rule:` or `option:` status therefore changes what `init` writes for the settings that use it; the fixtures under `tests/fixtures/init/` show the result.

## Equivalent ids of rules
`sudsy rules <id>` and the table in [rules/builtin.md](../rules/builtin.md) list as `Equivalent` every `covered` or `partial` entry whose status is `rule <id>` in a catalog for one of the rule's languages (`languages::catalog::equivalents`). Rule descriptions do not name other tools; add or fix the entry here instead.
