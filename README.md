<p align="center"><img src="assets/logo/soap_3d.png" alt="Sudsy" width="96"></p>

<h1 align="center">Sudsy</h1>

<p align="center"><b>One formatter and linter for your whole repo.</b><br>
One binary and one config file in place of Prettier, ESLint, ruff, buf, terraform fmt and the rest, measured file by file against each of them.<br>
<b><!-- generated:support-headline -->Ready for 7 languages, early support for 20 more.<!-- /generated --></b></p>

<p align="center">
<a href="https://github.com/bingsubandit/sudsy/releases"><img alt="release" src="https://img.shields.io/github/v/release/bingsubandit/sudsy"></a>
<a href="https://www.npmjs.com/package/@sudsylint/sudsy"><img alt="npm" src="https://img.shields.io/npm/v/@sudsylint/sudsy"></a>
<a href="https://pypi.org/project/sudsy/"><img alt="PyPI" src="https://img.shields.io/pypi/v/sudsy"></a>
<a href="LICENSE"><img alt="MIT" src="https://img.shields.io/badge/license-MIT-blue"></a>
</p>

> **Early release (v0.0.x).** <!-- generated:support-ready -->Ready: Protobuf, GraphQL, TOML, Angular templates, XML, Ruby and Makefile. The rest are in progress.<!-- /generated --> See the [status table](#language-support). Config may change in minor versions before 1.0.

## Install

| Channel | Command |
|---|---|
| npm | `npm i -D @sudsylint/sudsy` |
| PyPI | `pip install sudsy` or `uvx sudsy` |
| Homebrew | `brew install bingsubandit/sudsy/sudsy` |
| Scoop | `scoop bucket add sudsy https://github.com/bingsubandit/scoop-sudsy; scoop install sudsy` |
| Binary | [GitHub releases](https://github.com/bingsubandit/sudsy/releases): Linux, macOS and Windows, each for x64 and arm64 |

Or try it without installing. This is read-only: `check` reports and changes no files, and needs no config.

```sh
npx @sudsylint/sudsy check src/      # or: uvx sudsy check src/
```

```
a.py:1:1: error[format]: the file is not formatted; run `sudsy format`
a.py:1:8: warning[unused-import]: `os` imported but unused
a.py:3:6: warning[none-comparison]: compare to None with `is` or `is not`, not `==` or `!=`
b.js:3:18: warning[no-debugger]: Unexpected 'debugger' statement.
```

With no config, lint reports likely bugs only, never style. `sudsy format` rewrites the files. `sudsy fix` applies safe fixes, then formats.

Some languages are small downloads, fetched once with `sudsy languages add <name>` ([language packs](docs/configuration.md#language-packs)); until then their files are skipped with a warning. `sudsy languages list` shows what is installed. The `sudsy-full` release archives contain every language. Only C# needs more: it uses Roslyn in a worker process that every channel bundles, and the worker needs the [.NET 10 runtime](https://dotnet.microsoft.com/download/dotnet/10.0). Every other language works without .NET.

## Why

A typical repo runs one tool per language. Each tool has its own install, config file, version pins, editor plugin and CI step. Sudsy is one of each:

```diff
- npx prettier --check . && npx eslint . && ruff format --check && ruff check \
-   && shfmt -d . && buf format -d && terraform fmt -check && markdownlint '**/*.md' \
-   && yamllint . && hadolint Dockerfile
+ sudsy check
```

- **Keep your current style.** A preset reproduces each tool's output. The [table below](#language-support) shows how closely, measured on thousands of real-world files per language. Presets are plain config, and every style choice is a named option.
- **Editor and CI built in.** It includes a language server (with a VS Code extension) and outputs GitHub annotations and SARIF.

**Who it is for:** repos with several languages, where one tool and one config replace five or ten. It fits best today when most of the code is in the ready languages, for example a TypeScript and Python service with Protobuf APIs and Terraform infrastructure. **Who it is not for yet:** a JavaScript-only or Python-only repo already on Biome or ruff. Those tools are faster, and Sudsy adds nothing there except the same config across languages.

## Language support

Each number is measured on real-world code against the reference tool with its default settings, and counts every difference except the reviewed intentional ones. `Ready` means every number is at least 85%.

<!-- generated:support -->
| Language | Format | Lint | Status |
|---|---|---|---|
| Protobuf | █████████░ 99% buf format | ████████░░ 85% buf | Ready |
| GraphQL | █████████░ 99% Prettier | 21 rules · reference tool: none found | Ready |
| TOML | █████████░ 98% taplo | 15 rules · reference tool: none found | Ready |
| Angular templates | █████████░ 97% Prettier | 142 rules · reference tool: none found | Ready |
| XML | █████████░ 99% CSharpier XML | 17 rules · reference tool: none found | Ready |
| Ruby | none | █████████░ 96% RuboCop | Lint only: Ready |
| Makefile | Sudsy formatter · reference tool: none found | ██████████ 100% checkmake | Lint only: Ready |
| JavaScript / TypeScript | ████████░░ 84% Prettier | ██████████ 100% ESLint<br>██████████ 100% typescript-eslint | In progress |
| Python | ███████░░░ 79% ruff format | █████████░ 94% ruff | In progress |
| Terraform (HCL) | █████████░ 99% terraform fmt | █████░░░░░ 58% tflint | In progress |
| HTML | █████████░ 91% Prettier | ░░░░░░░░░░ 0% html-eslint<br>██████░░░░ 62% html-validate<br>░░░░░░░░░░ 5% htmlhint | In progress |
| Vue / Svelte | ████████░░ 85% Prettier | ██░░░░░░░░ 23% eslint-plugin-vue<br>██████████ 100% eslint-plugin-svelte | In progress |
| CSS / SCSS / Less | ████████░░ 87% Prettier | █████████░ 94% stylelint<br>not measured stylelint-scss<br>not measured stylelint-less | In progress |
| Markdown | ████████░░ 89% Prettier | ████████░░ 83% markdownlint | In progress |
| YAML | ██████████ 100% Prettier | ███████░░░ 74% yamllint | In progress |
| JSON / JSONC | ██████████ 100% Prettier<br>██████████ 100% Biome | not measured eslint-plugin-jsonc | In progress |
| PHP | ████████░░ 88% PHP-CS-Fixer | not measured PHPStan | In progress |
| Java | █████████░ 91% google-java-format | ██░░░░░░░░ 23% Checkstyle | In progress |
| Kotlin | ████████░░ 80% ktlint | █░░░░░░░░░ 11% ktlint | In progress |
| Shell | ████████░░ 81% shfmt | ████████░░ 86% ShellCheck | In progress |
| SQL | ███████░░░ 78% sqlfluff | ░░░░░░░░░░ 3% sqlfluff | In progress |
| C# | ██████████ 100% CSharpier | not measured StyleCop<br>not measured Roslynator | In progress |
| C / C++ | █░░░░░░░░░ 18% clang-format | 25 rules · reference tool: none found | In progress |
| Dockerfile | Sudsy formatter · reference tool: none found | ███████░░░ 78% hadolint | Lint only: In progress |
| GitHub Actions | Sudsy formatter · reference tool: none found | ░░░░░░░░░░ 5% actionlint | Lint only: In progress |
| PowerShell | not measured Invoke-Formatter | not measured PSScriptAnalyzer | not measured |
| INI, EditorConfig, .gitignore | Sudsy formatter · reference tool: none found | 24 rules · reference tool: none found | not measured |

How to read the numbers, what they leave out and how to reproduce one: [docs/languages.md](docs/languages.md#how-to-read-the-numbers). Presets: [configuration](docs/configuration.md#presets).
<!-- /generated -->

All languages: <!-- generated:languages -->C#, C/C++, CSS, Dockerfile, EditorConfig, Gitignore, GraphQL, HCL, HTML, INI, JSON, Java, JavaScript/TypeScript, Kotlin, Makefile, Markdown, PHP, PowerShell, Protobuf, Python, Ruby, SQL, Shell, TOML, XML and YAML<!-- /generated -->. Some are [language packs](#install). Corpora, tool versions and the main differences per language: [docs/languages.md](docs/languages.md).

## Beyond parity

<!-- generated:beyond -->
Sudsy format options vs other tools:

| Language | Sudsy options | Reference tool | Tool options |
|---|---:|---|---:|
| JavaScript / TypeScript | 28 | Prettier | [17](https://prettier.io/docs/options) |
| CSS / SCSS / Less | 7 | Prettier | [5](https://prettier.io/docs/options) |
| YAML | 14 | Prettier | [7](https://prettier.io/docs/options) |
| GraphQL | 6 | Prettier | [5](https://prettier.io/docs/options) |
| Python | 12 | ruff format | [9](https://docs.astral.sh/ruff/settings/#format) |
| Shell | 8 | shfmt | [6](https://github.com/mvdan/sh/blob/master/cmd/shfmt/shfmt.1.scd) |
| Java | 23 | google-java-format | [1](https://github.com/google/google-java-format#using-the-formatter) |
| Protobuf | 4 | buf format | [0](https://buf.build/docs/format/style) |
| Terraform (HCL) | 3 | terraform fmt | [0](https://developer.hashicorp.com/terraform/cli/commands/fmt) |
| C# | 64 | CSharpier | [5](https://csharpier.com/docs/Options) |
<!-- /generated -->

- **Your own rules:** [YAML patterns](docs/rules.md#pattern-rules) with fixes, and sandboxed [Wasm plugins](docs/rules.md#wasm-plugins).
- **One config for every language:** one [`sudsy.yaml`](docs/configuration.md), with `overrides` per path and presets that reproduce each tool.
- **Safe fixes:** `sudsy fix` applies only [safe fixes](docs/rules.md#fixes), re-parses after each round and undoes a round that leaves the file unparsable.
- **Meaning check:** tests and corpus runs [compare the syntax tree before and after formatting](docs/ai/formatting/engine.md) and fail on any change, and on a second format that changes anything.

## Performance

<!-- generated:speed -->
| Language | Task | Compared with | Sudsy speed | Time (Sudsy / tool) | Peak memory (Sudsy / tool) |
|---|---|---|---|---|---|
| JSON | Format | Prettier | 26× faster | 0.009 s / 0.23 s | 43 / 85 MB |
|  |  | Biome | 4.1× faster | 0.009 s / 0.037 s | 43 / 42 MB |
|  |  | dprint | 1.7× faster | 0.007 s / 0.012 s | 73 / 73 MB |
|  | Lint | eslint-plugin-jsonc | not measured: no mirroring preset | - | - |
| YAML | Format | Prettier | 43× faster | 0.15 s / 6.48 s | 103 / 237 MB |
|  |  | dprint | 1.4× faster | 0.17 s / 0.23 s | 99 / 73 MB |
|  | Lint | yamllint | 18× faster | 0.071 s / 1.25 s | 62 / 55 MB |
| JS/TS | Format | Prettier | 23× faster | 0.21 s / 4.87 s | 172 / 298 MB |
|  |  | Biome | 1.5× slower | 0.27 s / 0.18 s | 158 / 104 MB |
|  | Lint | ESLint | 16× faster | 0.27 s / 4.30 s | 189 / 451 MB |
|  |  | typescript-eslint | 24× faster | 0.32 s / 7.54 s | 240 / 730 MB |
|  |  | Biome | 1.4× faster | 0.27 s / 0.38 s | 205 / 146 MB |
|  |  | oxlint | 4.2× slower | 0.24 s / 0.057 s | 199 / 160 MB |
| CSS | Format | Prettier | 39× faster | 0.14 s / 5.61 s | 157 / 392 MB |
|  | Lint | stylelint | 7.5× faster | 0.27 s / 2.04 s | 113 / 355 MB |
| HTML | Format | Prettier | 54× faster | 0.026 s / 1.40 s | 98 / 169 MB |
|  | Lint | html-validate | 25× faster | 0.090 s / 2.27 s | 104 / 285 MB |
|  |  | HTMLHint | 4.0× faster | 0.066 s / 0.26 s | 82 / 100 MB |
|  |  | html-eslint | 2.0× faster | 0.062 s / 0.12 s | 82 / 75 MB |
| Angular | Format | Prettier | not measured: Prettier picks its Angular parser from the .component.html name, which the corpus files do not carry | - | - |
| TOML | Format | taplo | 1.6× faster | 0.012 s / 0.019 s | 45 / 40 MB |
| Python | Format | ruff | 4.8× slower | 0.31 s / 0.066 s | 224 / 58 MB |
|  | Lint | ruff | 2.1× slower | 0.34 s / 0.17 s | 245 / 57 MB |
| SQL | Format | sqlfluff | 386× faster | 0.012 s / 4.63 s | 57 / 52 MB |
|  | Lint | sqlfluff | 122× faster | 0.043 s / 5.27 s | 52 / 52 MB |
| Shell | Format | shfmt | 2.6× slower | 0.059 s / 0.023 s | 79 / 52 MB |
|  | Lint | ShellCheck | 1913× faster | 0.089 s / 170.25 s | 86 / 9735 MB |
| Java | Format | google-java-format | 19× faster | 1.09 s / 20.53 s | 646 / 998 MB |
|  | Lint | Checkstyle | 153× faster | 0.96 s / 146.16 s | 591 / 1173 MB |
| Kotlin | Format | ktlint | 30× faster | 1.61 s / 48.42 s | 501 / 715 MB |
|  | Lint | ktlint | 42× faster | 1.14 s / 48.01 s | 485 / 683 MB |
| PHP | Format | PHP-CS-Fixer | 161× faster | 1.77 s / 285.46 s | 1161 / 70 MB |
|  | Lint | PHPStan | not measured: the analysis runs per project folder, not on a folder of unrelated files | - | - |
| PowerShell | Format | Invoke-Formatter | not measured: no PowerShell (pwsh) on the host that wrote this table | - | - |
|  | Lint | PSScriptAnalyzer | not measured: no PowerShell (pwsh) on the host that wrote this table | - | - |
| Protobuf | Format | buf | 3.6× faster | 0.069 s / 0.25 s | 118 / 74 MB |
|  | Lint | buf | not measured: the analysis runs per module, not on a folder of unrelated files | - | - |
| HCL | Format | terraform fmt | 3.5× faster | 0.11 s / 0.40 s | 123 / 87 MB |
|  | Lint | tflint | 23× faster | 0.20 s / 4.69 s | 120 / 89 MB |
| C# | Format | CSharpier | 3.3× faster | 4.36 s / 14.38 s | 572 / 456 MB |
|  | Lint | StyleCop | not measured: the compiler skips analyzers when the compilation has errors, and the C# corpus does not compile on its own | - | - |
| XML | Format | CSharpier | 4.0× faster | 0.11 s / 0.44 s | 104 / 97 MB |
| C# (.editorconfig) | Format | dotnet format | not measured: its fixtures are small .editorconfig cases, each in a folder of its own, not a corpus | - | - |
| Ruby | Lint | RuboCop | 88× faster | 3.00 s / 264.55 s | 1180 / 377 MB |
| Vue | Lint | eslint-plugin-vue | not measured: no runner yet: the flat config for .vue files needs vue-eslint-parser and a corpus of .vue files | - | - |
| Svelte | Lint | eslint-plugin-svelte | not measured: no runner yet: the flat config for .svelte files needs svelte-eslint-parser and a corpus of .svelte files | - | - |
| SCSS | Lint | stylelint-scss | not measured: no mirroring preset | - | - |
| Less | Lint | stylelint-less | not measured: no mirroring preset | - | - |
| Markdown | Lint | markdownlint | 7.5× faster | 0.096 s / 0.72 s | 87 / 198 MB |
| Dockerfile | Lint | hadolint | 36× faster | 0.071 s / 2.57 s | 70 / 74 MB |
| GitHub Actions | Lint | actionlint | 2.1× slower | 0.14 s / 0.065 s | 76 / 71 MB |
| Makefile | Lint | checkmake | 3.7× slower | 0.059 s / 0.016 s | 70 / 28 MB |

Method, runner and corpora: [docs/performance.md](docs/performance.md).
<!-- /generated -->

Sudsy does the same work as each tool: `format --check` and `lint` with the preset that mirrors it. The binary is about 53 MB: it holds the parsers for every language and a Wasm runtime for plugins.

## Configure

```sh
sudsy init --preset python/ruff     # a starter sudsy.yaml with one or more presets
sudsy init --from prettier          # read your .prettierrc and write the matching options
```

`--from` reads the config of Prettier, Biome, ESLint, stylelint, yamllint, CSharpier, StyleCop, html-validate, htmlhint and html-eslint. For ruff, use the `python/ruff` preset and set its options by hand.

```yaml
# sudsy.yaml
extends: [preset:js/eslint-recommended, preset:python/ruff, preset:java/google]
exclude: ["**/generated/**"]
python:
  format: { line_width: 100 }
overrides:
  - files: ["tests/**"]
    python: { lint: { rules: { unused-import: "off" } } }
```

Existing `.editorconfig` keys apply. `sudsy config print` shows every resolved value and where it came from. `sudsy rules <id>` explains a rule. Reference: [docs/configuration.md](docs/configuration.md).

## CI and editors

```sh
sudsy check --format github                # annotations in GitHub Actions
sudsy check --format sarif > sudsy.sarif   # GitHub code scanning
```

`check` exits 1 when a file is unformatted or a rule reports an error. `sudsy lsp` is a language server over stdio, with diagnostics, quick fixes and formatting. VS Code: install the `.vsix` for your platform from the [release](https://github.com/bingsubandit/sudsy/releases) with `code --install-extension sudsy-<platform>-<version>.vsix`. See [docs/editors-and-ci.md](docs/editors-and-ci.md).

## More

- [Languages](docs/languages.md): status, options and presets per language
- [Configuration](docs/configuration.md): `sudsy.yaml`, presets, `.editorconfig`, the CLI
- [Rules](docs/rules.md): built-in rules, pattern rules, Wasm plugins
- [Editors and CI](docs/editors-and-ci.md) and [performance](docs/performance.md)
- [Roadmap](docs/ai/roadmap.md), [changelog](CHANGELOG.md), [contributing](CONTRIBUTING.md)

MIT licensed. Test inputs from other projects: [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).
