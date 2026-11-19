# Configuration

Sudsy reads one `sudsy.yaml` (or `sudsy.yml`) for all languages. The nearest file above the working directory applies. An empty file means the defaults.

```sh
sudsy init --preset python/ruff   # write a starter sudsy.yaml
sudsy config print src/app.py     # show every resolved value and its source
```

## Example

```yaml
# sudsy.yaml
extends: [preset:js/eslint-recommended, preset:python/ruff, ./base.yaml]
include: ["src/**"]               # globs relative to this file; empty means everything
exclude: ["**/generated/**"]      # in addition to .gitignore
analysis: standard                # fast | standard (default) | deep
format: { line_width: 100 }       # shared by every language
languages: { toml: required, markdown: ignored }
plugins: { patterns: ["rules/*.yaml"], wasm: ["rules/team.wasm"] }
python:
  format: { quote_style: single }
  lint:
    rules:
      no-print: warn
      max-line-length: { level: warn, options: { max: 100 } }
overrides:
  - files: ["tests/**"]
    python: { lint: { rules: { no-print: "off" } } }
```

- Unknown keys, unknown rule ids and invalid values are errors. The message gives file, line and column.
- Booleans are only `true` and `false`. `off` is a plain string.
- Explicit file arguments bypass `include` and `exclude`.
- Config files over 1 MiB and plugin `.wasm` files over 32 MiB are refused.

## Presets

A preset is an ordinary config file built into the binary. Write its values by hand to get the same result. Presets are unversioned: `preset:csharp/google@1` is an error.

`extends: [preset:<language>/<name>]`. Presets are built into the binary.

| Language | Presets |
|---|---|
| C# | `csharp/csharpier-compat`, `csharp/dotnet-runtime`, `csharp/google`, `csharp/stylecop`, `csharp/unity` |
| CSS, SCSS, Less | `css/stylelint-recommended`, `css/stylelint-standard` |
| Dockerfile | `dockerfile/hadolint` |
| GitHub Actions | `github-actions/actionlint` |
| HCL (Terraform) | `hcl/tflint-recommended` |
| HTML, Vue, Svelte | `html/html-validate-recommended`, `html/html-validate-standard`, `html/htmlhint`, `html/html-eslint-recommended`, `html/eslint-plugin-vue-recommended`, `html/eslint-plugin-svelte-recommended` |
| Java | `java/google` |
| JavaScript, TypeScript | `js/eslint-recommended`, `js/typescript-eslint-recommended`, `js/biome`, `js/biome-recommended`, `js/oxlint` |
| JSON | `json/prettier-compat`, `json/biome`, `json/dprint` |
| Kotlin | `kotlin/ktlint` |
| Makefile | `makefile/checkmake` |
| Markdown | `markdown/prettier-compat`, `markdown/markdownlint` |
| PHP | `php/phpstan` |
| PowerShell | `powershell/psscriptanalyzer` |
| Protobuf | `protobuf/buf-standard` |
| Python | `python/ruff` |
| Ruby | `ruby/rubocop` |
| Shell | `shell/shellcheck` |
| SQL | `sql/sqlfluff` |
| TOML | `toml/taplo-compat` |
| XML | `xml/csharpier-compat` |
| YAML | `yaml/prettier-compat`, `yaml/yamllint` |

Without `extends`, each language uses Sudsy's defaults. For C# the default is Microsoft's layout.

What each preset matches, and how closely: [languages](languages.md).

## Keep your current style

Every parity number in [languages](languages.md) compares against the reference tool with its **default** config. If your project sets other options, copy them into `sudsy.yaml`. Then check the result on your code: the percentages do not apply to custom configs.

| You use | Do this | Carries over | Does not carry over |
|---|---|---|---|
| Prettier, Biome, ESLint, stylelint | `sudsy init --from <tool>` | The preset (for example `json/prettier-compat`), width, tabs, quotes and other mapped options, rule levels and options that have a Sudsy rule, ignore files as `exclude`. | Plugins, shared configs and rules Sudsy lacks. The command prints each one as `not migrated`. A JavaScript config file (`eslint.config.*`) is refused. |
| ruff | `extends: [preset:python/ruff]`, then set options by hand: `line_width`, `quote_style`, `magic_trailing_comma`, `target_version` | The formatter style and ruff's default lint set (95.6% of its findings on the corpus). | `pyproject.toml` is not read. Rules you selected beyond ruff's defaults are not mapped. No `init --from ruff`. |
| Black | `extends: [preset:python/ruff]` | Black style, through ruff format. Set `line_width` if you changed Black's 88. | `pyproject.toml` is not read. The comparison runs ruff format, not Black itself. |

`init --from` accepts: `yamllint`, `prettier`, `biome`, `eslint`, `stylelint`, `stylecop`, `csharpier`, `html-validate`, `htmlhint`, `html-eslint`. It writes only the differences from the extended presets. It replaces an existing `sudsy.yaml` only with `--force`. Details: [ai/config/cli.md](ai/config/cli.md#init---from).

## Where values come from

Highest priority first:

1. CLI flags (`--analysis`).
2. A matching `overrides` entry.
3. The language section (`python:`).
4. Top-level `format:` and `lint:`.
5. The `extends` chain. Presets rank below all files.
6. `.editorconfig`.
7. Built-in defaults.

Run `sudsy config print [file]` to see the result. Each value shows its source: `file:line`, `preset <name>`, `.editorconfig` or `default`.

## Format options

Set them under `format:` at the top level (all languages) or in a language section.

- Shared keys: `line_width`, `indent_size`, `indent_style`, `final_newline`, `end_of_line`, `bracket_spacing`, `array_bracket_spacing`. A language ignores a shared key it does not declare. Any other key at this level is an error.
- Each language adds its own keys. They are listed in [languages](languages.md) and in the editor schema.
- Section names are language names. List them with `sudsy languages list`: `js`, `python`, `csharp`, `hcl`, `shell`, and so on.

## Lint rules

```yaml
python:
  lint:
    rules:
      no-print: warn                                  # off | warn | error
      max-line-length: { level: warn, options: { max: 100 } }
```

- Levels are `off`, `warn` and `error`.
- With no config, only rules that catch likely bugs are on. Style and naming rules are off.
- A preset turns on the rules that match a tool. Run `sudsy rules <id>` to see a rule's options and the presets that enable it.
- A rule named in another language's section is ignored there.

### Analysis levels

`analysis` sets how deep lint looks. The CLI flag `--analysis` overrides it.

| Level | What runs |
|---|---|
| `fast` | Syntax-only rules. |
| `standard` (default) | Adds rules that need semantic analysis of one file or project. |
| `deep` | Adds project-wide rules. |

A rule that cannot run at the chosen level is skipped, not failed.

## Overrides

`overrides` change settings for matching files. Each entry takes `files` and any `format`, `lint` or language section.

```yaml
overrides:
  - files: ["tests/**", "**/*_test.py"]
    python: { lint: { rules: { no-print: "off" } } }
```

## Suppression comments

Use the language's own comment syntax. Rule names are optional: with none, every rule is silenced.

| Comment | Effect |
|---|---|
| `sudsy-disable-next-line [rules]` | The next line. |
| `sudsy-disable-line [rules]` | The line the comment ends on. |
| `sudsy-disable [rules]` ... `sudsy-enable [rules]` | A block. Without `enable`, to the end of the file. |
| `sudsy-disable-file [rules]` | The whole file. |

```python
# sudsy-disable-next-line unused-import
import sys
```

Plain JSON has no comments. Use `overrides` with `"off"` there.

## .editorconfig

`.editorconfig` is a fallback below your `extends` chain. Sudsy walks up from the file until a file says `root = true`.

| `.editorconfig` key | Sudsy option |
|---|---|
| `indent_style` | `indent_style` |
| `indent_size`, `tab_width` | `indent_size` |
| `max_line_length` | `line_width` |
| `insert_final_newline` | `final_newline` |
| `end_of_line` (`lf`, `crlf`) | `end_of_line` |
| `csharp_new_line_before_*`, `csharp_indent_labels`, `csharp_preserve_single_line_*`, `dotnet_sort_system_directives_first` and related | C# options (see [details](ai/config/editorconfig.md)) |

- Invalid values are ignored, not errors.
- A `csharp_space_*` key that Sudsy does not follow prints one warning per line on stderr.
- Other keys belong to other tools and are ignored.

## Language packs

Some languages are small downloads, called packs. Fetch them once:

```sh
sudsy languages list                  # installed or available, with pack size
sudsy languages add toml protobuf     # download packs for this version
sudsy languages sync                  # install what `languages:` marks required
sudsy languages remove toml
```

- Packs: editorconfig, gitignore, graphql, hcl, ini, markdown, powershell, protobuf, shell, toml, xml, dockerfile and makefile. The others are built in. The list is from `sudsy languages list` on 0.0.2.
- Files of a language whose pack is missing are skipped. Sudsy prints one warning per language: `warning: 1 toml file(s) skipped: not installed; run `sudsy languages add toml``.
- Exit code: if nothing else went wrong, `format`, `lint`, `fix` and `check` exit with 3 after a skip. Exit 3 is used only when everything else succeeded (verified: one skipped TOML file and one clean Python file gave 3). A language marked `ignored` is skipped silently and does not change the exit code.
- `sudsy.yaml` marks a language: `languages: { toml: required, markdown: ignored }`. `required` is what `sync` installs.
- The `sudsy-full-<version>-<platform>` archives on the GitHub release contain every pack. Use one for offline machines or to avoid the download.
- In CI, run `sudsy languages add <names>` once before `check`, or use a full archive. See [editors and CI](editors-and-ci.md#github-actions).

## Commands

| Command | What it does | Exit code |
|---|---|---|
| `sudsy format [--check] [paths]` | Format files. `--check` writes nothing. | 1 if `--check` finds changes |
| `sudsy lint [paths]` | Report diagnostics. | 1 on any error |
| `sudsy fix [--unsafe] [paths]` | Apply fixes, then format. | 1 if errors remain |
| `sudsy check [paths]` | `format --check` plus `lint`. For CI. | 1 on any error or unformatted file |
| `sudsy rules [term] [--language <lang>]` | List rules, explain one by id, or search. | 2 if nothing matches |
| `sudsy config print [file]` | Show resolved config and sources. | 0 |
| `sudsy init [--preset <name>] [--force]` | Write a starter `sudsy.yaml`. | 0 |
| `sudsy languages list\|add\|remove\|sync` | Manage language packs. | 2 on failure |
| `sudsy lsp` | Language server on stdio. | |

Paths default to the current directory. `.gitignore` is honored, also outside a git checkout.

Exit codes for `format`, `lint`, `fix` and `check`:

- `0`: success.
- `1`: lint errors or unformatted files.
- `2`: a file could not be processed, or the config is invalid. Other files are still processed.
- `3`: everything else worked, but files of a language whose pack is missing were skipped. See [language packs](#language-packs).

`lint`, `fix` and `check` take `--format human|json|sarif|github` and `--analysis fast|standard|deep`.

| Format | Use |
|---|---|
| `human` (default) | Terminal. `path:line:col: severity[rule]: message`. |
| `json` | Scripts. Object with `version`, `summary`, `diagnostics`. Has byte offsets and fix edits. |
| `sarif` | GitHub code scanning. SARIF 2.1.0, without fixes. |
| `github` | Annotations in GitHub Actions. |

`format` and `fix` write atomically, keep file permissions and keep a UTF-8 BOM.

## Reference

JSON Schema for editors: `schemas/sudsy.schema.json`, also shipped with the VS Code extension. Full design notes: [ai/config/sudsy-yaml.md](ai/config/sudsy-yaml.md).
