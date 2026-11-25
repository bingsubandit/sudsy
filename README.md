<p align="center"><img src="assets/logo/soap_3d.png" alt="Sudsy" width="96"></p>

<h1 align="center">Sudsy</h1>

<p align="center"><b>A true polyglot formatter and linter for every repo.</b><br>
One binary and one config file in place of Prettier, ESLint, ruff, buf, terraform fmt and the rest.<br>
Extensible: add your own rules as YAML patterns or Wasm plugins.<br>
<b><!-- generated:support-headline -->Ready for 27 languages, early support for 2 more.<!-- /generated --></b></p>

<p align="center">
<a href="https://github.com/bingsubandit/sudsy/releases"><img alt="release" src="https://img.shields.io/github/v/release/bingsubandit/sudsy"></a>
<a href="https://www.npmjs.com/package/@sudsylint/sudsy"><img alt="npm" src="https://img.shields.io/npm/v/@sudsylint/sudsy"></a>
<a href="https://pypi.org/project/sudsy/"><img alt="PyPI" src="https://img.shields.io/pypi/v/sudsy"></a>
<a href="https://github.com/bingsubandit/homebrew-sudsy"><img alt="Homebrew" src="https://img.shields.io/badge/dynamic/regex?url=https%3A%2F%2Fraw.githubusercontent.com%2Fbingsubandit%2Fhomebrew-sudsy%2FHEAD%2FFormula%2Fsudsy.rb&amp;search=version%20%22(%5B%5E%22%5D%2B)%22&amp;replace=v%241&amp;label=homebrew"></a>
<a href="https://github.com/bingsubandit/scoop-sudsy"><img alt="Scoop" src="https://img.shields.io/scoop/v/sudsy?bucket=https://github.com/bingsubandit/scoop-sudsy"></a>
<a href="https://marketplace.visualstudio.com/items?itemName=sudsy.sudsy"><img alt="VS Code Marketplace" src="https://vsmarketplacebadges.dev/version-short/sudsy.sudsy.svg"></a>
<a href="https://open-vsx.org/extension/sudsy/sudsy"><img alt="Open VSX" src="https://img.shields.io/open-vsx/v/sudsy/sudsy"></a>
<a href="LICENSE"><img alt="MIT" src="https://img.shields.io/badge/license-MIT-blue"></a>
</p>

> **Early release (v0.0.x).** <!-- generated:support-ready -->Ready: HTML, JavaScript / TypeScript, CSS / SCSS / Less, JSON / JSONC, Markdown, Python, YAML, Shell, Dockerfile, SQL, Java, GitHub Actions, C#, PowerShell, Makefile, TOML, XML, Go, CMake, Kotlin, Vue / Svelte, Terraform (HCL), Angular templates, INI / EditorConfig / .gitignore, Ruby, GraphQL and Protobuf. The rest are in progress.<!-- /generated --> See the [status table](#language-support). Config may change in minor versions before 1.0.

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

**Who it is for:** repos with several languages, where one tool and one config replace five or ten. It fits best today when most of the code is in the ready languages, for example a TypeScript and Python service with Protobuf APIs and Terraform infrastructure.

## Language support

Each number is measured on real-world code against the default rule set of the reference tool (formatters: its default settings) and counts every difference except the reviewed intentional ones. A row measured with another set says so in the notes under the table. How each language is measured: [docs/languages.md](docs/languages.md). Ready languages come first, then the others, each by how widely the language is used. `Ready` means the first tool of each column is at least 85%; the others are shown for reference. Where no reference formatter exists, `stable` is the share of real-world files that Sudsy formats without error, to the same text on a second run and by its rules rather than copied through.

<!-- generated:support -->
| Language | Format | Lint | Status |
|---|---|---|---|
| HTML | █████████░ 95% Prettier | █████████░ 92% html-validate<br>████████░░ 88% html-eslint<br>█████████░ 91% htmlhint | Ready |
| JavaScript / TypeScript | █████████░ 95% Prettier | ██████████ 100% ESLint<br>██████████ 100% typescript-eslint | Ready |
| CSS / SCSS / Less | █████████░ 92% Prettier | █████████░ 94% stylelint<br>████████░░ 89% stylelint-scss<br>████████░░ 85% stylelint-less | Ready |
| JSON / JSONC | █████████░ 99% Prettier<br>█████████░ 99% Biome | ██████████ 100% eslint-plugin-jsonc | Ready |
| Markdown | █████████░ 92% Prettier | █████████░ 99% markdownlint | Ready |
| Python | █████████░ 97% ruff format | █████████░ 94% ruff | Ready |
| YAML | █████████░ 94% Prettier | █████████░ 98% yamllint | Ready |
| Shell | ████████░░ 86% shfmt | ████████░░ 86% ShellCheck | Ready |
| Dockerfile | ██████████ 100% stable · no reference tool | █████████░ 91% hadolint | Ready |
| SQL | ████████░░ 85% sqlfluff | ████████░░ 88% sqlfluff | Ready |
| Java | █████████░ 91% google-java-format | ████████░░ 88% Checkstyle | Ready |
| GitHub Actions | █████████░ 99% stable · no reference tool | ████████░░ 87% actionlint | Ready |
| C# | ████████░░ 85% CSharpier | ████████░░ 87% StyleCop<br>░░░░░░░░░░ 0% Roslynator | Ready |
| PowerShell | █████████░ 92% Invoke-Formatter | ████████░░ 87% PSScriptAnalyzer | Ready |
| Makefile | ██████████ 100% stable · no reference tool | ██████████ 100% checkmake | Ready |
| TOML | █████████░ 98% taplo | 22 rules · no reference tool | Ready |
| XML | ██████████ 100% CSharpier XML | 24 rules · no reference tool | Ready |
| Go | none | █████████░ 97% revive | Lint only: Ready |
| CMake | █████████░ 90% gersemi | █████████░ 97% cmake-lint | Ready |
| Kotlin | ████████░░ 85% ktlint | █████████░ 92% ktlint | Ready |
| Vue / Svelte | █████████░ 91% Prettier | ████████░░ 88% eslint-plugin-vue<br>██████████ 100% eslint-plugin-svelte | Ready |
| Terraform (HCL) | █████████░ 99% terraform fmt | █████████░ 93% tflint | Ready |
| Angular templates | █████████░ 99% Prettier | █████████░ 99% angular-eslint | Ready |
| INI / EditorConfig / .gitignore | ██████████ 100% stable · no reference tool | 31 rules · no reference tool | Ready |
| Ruby | none | █████████░ 96% RuboCop | Lint only: Ready |
| GraphQL | █████████░ 99% Prettier | 29 rules · no reference tool | Ready |
| Protobuf | █████████░ 99% buf format | ████████░░ 85% buf | Ready |
| C / C++ | ██████░░░░ 69% clang-format | not measured clang-tidy | In progress |
| PHP | ████████░░ 88% PHP-CS-Fixer | ███░░░░░░░ 36% PHPStan | In progress |

Notes: clang-format (C / C++): line wrapping is not compared. PHP-CS-Fixer (PHP): measured with the `@PER-CS` rule set (the default of PHP-CS-Fixer 3 is `@PSR12`, which `@PER-CS` contains).

The rows are ordered by how widely the language is used. Every number is measured against each reference tool's default rule set; a row measured with another set says so in the notes above. How each language is measured, with the tool versions and corpora: [docs/languages.md](docs/languages.md#how-to-read-the-numbers). Presets: [configuration](docs/configuration.md#presets).
<!-- /generated -->

All languages: <!-- generated:languages -->HTML, JavaScript/TypeScript, CSS, JSON, Markdown, Python, YAML, Shell, Dockerfile, SQL, Java, C#, PowerShell, Makefile, TOML, XML, Go, CMake, Kotlin, HCL, EditorConfig, Gitignore, INI, Ruby, GraphQL, Protobuf, C/C++ and PHP<!-- /generated -->. Corpora, tool versions and the main differences per language: [docs/languages.md](docs/languages.md).

## Highly configurable

<!-- generated:beyond -->
**Extensive formatting and lint rules.** Sudsy format options vs other tools:

| Language | Sudsy options | Reference tool | Tool options |
|---|---:|---|---:|
| HTML | 17 | Prettier | [8](https://prettier.io/docs/options) |
| JavaScript / TypeScript | 33 | Prettier | [17](https://prettier.io/docs/options) |
| CSS / SCSS / Less | 7 | Prettier | [6](https://prettier.io/docs/options) |
| JSON / JSONC | 10 | Prettier | [7](https://prettier.io/docs/options) |
| JSON / JSONC | 10 | Biome | [9](https://biomejs.dev/reference/configuration/#json) |
| Markdown | 5 | Prettier | [5](https://prettier.io/docs/options) |
| Python | 13 | ruff format | [11](https://docs.astral.sh/ruff/settings/#format) |
| YAML | 14 | Prettier | [7](https://prettier.io/docs/options) |
| Shell | 8 | shfmt | [8](https://github.com/mvdan/sh/blob/v3.14.1/cmd/shfmt/shfmt.1.scd) |
| SQL | 14 | sqlfluff | [68](https://docs.sqlfluff.com/en/stable/configuration/default_configuration.html) |
| Java | 23 | google-java-format | [8](https://github.com/google/google-java-format#from-the-command-line) |
| C# | 93 | CSharpier | [4](https://csharpier.com/docs/Configuration) |
| PowerShell | 5 | Invoke-Formatter | [33](https://github.com/PowerShell/PSScriptAnalyzer/tree/1.25.0/docs/Rules) |
| TOML | 11 | taplo | [19](https://taplo.tamasfe.dev/configuration/formatter-options.html) |
| XML | 6 | CSharpier XML | [5](https://csharpier.com/docs/Configuration) |
| CMake | 7 | gersemi | [4](https://github.com/BlankSpruce/gersemi#configuration) |
| Kotlin | 45 | ktlint | [101](https://github.com/pinterest/ktlint/blob/1.8.0/documentation/release-latest/docs/rules/configuration-ktlint.md) |
| Vue / Svelte | 17 | Prettier | [18](https://prettier.io/docs/options) |
| Terraform (HCL) | 3 | terraform fmt | [0](https://developer.hashicorp.com/terraform/cli/commands/fmt) |
| Angular templates | 16 | Prettier | [15](https://prettier.io/docs/options) |
| GraphQL | 6 | Prettier | [5](https://prettier.io/docs/options) |
| Protobuf | 4 | buf format | [0](https://buf.build/docs/reference/cli/buf/format/) |
| C / C++ | 48 | clang-format | [257](https://clang.llvm.org/docs/ClangFormatStyleOptions.html) |
| PHP | 7 | PHP-CS-Fixer | [390](https://cs.symfony.com/doc/rules/index.html) |
<!-- /generated -->

- **Your own rules:** [YAML patterns](docs/rules.md#pattern-rules) with fixes, and sandboxed [Wasm plugins](docs/rules.md#wasm-plugins).
- **One config for every language:** one [`sudsy.yaml`](docs/configuration.md), with `overrides` per path and presets that reproduce each tool.
- **Safe fixes:** `sudsy fix` applies only [safe fixes](docs/rules.md#fixes), re-parses after each round and undoes a round that leaves the file unparsable.
- **Meaning check:** tests and corpus runs [compare the syntax tree before and after formatting](docs/ai/formatting/engine.md) and fail on any change, and on a second format that changes anything.

## Performance

<!-- generated:speed -->
| Language | Task | Compared with | Sudsy speed | Time (Sudsy / tool) | Peak memory (Sudsy / tool) |
|---|---|---|---|---|---|
| HTML | Format | Prettier | 54× faster | 0.026 s / 1.40 s | 98 / 169 MB |
|  | Lint | html-validate | 25× faster | 0.090 s / 2.27 s | 104 / 285 MB |
|  |  | HTMLHint | 4.0× faster | 0.066 s / 0.26 s | 82 / 100 MB |
|  |  | html-eslint | 2.0× faster | 0.062 s / 0.12 s | 82 / 75 MB |
| JS/TS | Format | Prettier | 23× faster | 0.21 s / 4.87 s | 172 / 298 MB |
|  |  | Biome | 1.5× slower | 0.27 s / 0.18 s | 158 / 104 MB |
|  | Lint | ESLint | 16× faster | 0.27 s / 4.30 s | 189 / 451 MB |
|  |  | typescript-eslint | 24× faster | 0.32 s / 7.54 s | 240 / 730 MB |
|  |  | Biome | 1.4× faster | 0.27 s / 0.38 s | 205 / 146 MB |
|  |  | oxlint | 4.2× slower | 0.24 s / 0.057 s | 199 / 160 MB |
| CSS | Format | Prettier | 39× faster | 0.14 s / 5.61 s | 157 / 392 MB |
|  | Lint | stylelint | 7.5× faster | 0.27 s / 2.04 s | 113 / 355 MB |
| SCSS | Lint | stylelint-scss | 47× faster | 0.052 s / 2.43 s | 89 / 273 MB |
| Less | Lint | stylelint-less | 54× faster | 0.080 s / 4.29 s | 120 / 440 MB |
| JSON | Format | Prettier | 26× faster | 0.009 s / 0.23 s | 43 / 85 MB |
|  |  | Biome | 4.1× faster | 0.009 s / 0.037 s | 43 / 42 MB |
|  |  | dprint | 1.7× faster | 0.007 s / 0.012 s | 73 / 73 MB |
|  | Lint | eslint-plugin-jsonc | 1.4× faster | 0.87 s / 1.18 s | 866 / 75 MB |
| Markdown | Lint | markdownlint | 7.5× faster | 0.096 s / 0.72 s | 87 / 198 MB |
| Python | Format | ruff | 4.8× slower | 0.31 s / 0.066 s | 224 / 58 MB |
|  | Lint | ruff | 2.1× slower | 0.34 s / 0.17 s | 245 / 57 MB |
| YAML | Format | Prettier | 43× faster | 0.15 s / 6.48 s | 103 / 237 MB |
|  |  | dprint | 1.4× faster | 0.17 s / 0.23 s | 99 / 73 MB |
|  | Lint | yamllint | 18× faster | 0.071 s / 1.25 s | 62 / 55 MB |
| Shell | Format | shfmt | 2.6× slower | 0.059 s / 0.023 s | 79 / 52 MB |
|  | Lint | ShellCheck | 1913× faster | 0.089 s / 170.25 s | 86 / 9735 MB |
| Dockerfile | Lint | hadolint | 36× faster | 0.071 s / 2.57 s | 70 / 74 MB |
| SQL | Format | sqlfluff | 386× faster | 0.012 s / 4.63 s | 57 / 52 MB |
|  | Lint | sqlfluff | 122× faster | 0.043 s / 5.27 s | 52 / 52 MB |
| Java | Format | google-java-format | 19× faster | 1.09 s / 20.53 s | 646 / 998 MB |
|  | Lint | Checkstyle | 153× faster | 0.96 s / 146.16 s | 591 / 1173 MB |
| GitHub Actions | Lint | actionlint | 2.1× slower | 0.14 s / 0.065 s | 76 / 71 MB |
| C# | Format | CSharpier | 3.3× faster | 4.36 s / 14.38 s | 572 / 456 MB |
|  | Lint | StyleCop | not measured | - | - |
| C# (.editorconfig) | Format | dotnet format | 5.2× faster | 1.93 s / 10.05 s | 537 / 807 MB |
| PowerShell | Format | Invoke-Formatter | 38× faster | 0.11 s / 4.38 s | 165 / 163 MB |
|  | Lint | PSScriptAnalyzer | 151× faster | 0.13 s / 20.17 s | 136 / 313 MB |
| Makefile | Lint | checkmake | 3.7× slower | 0.059 s / 0.016 s | 70 / 28 MB |
| TOML | Format | taplo | 1.6× faster | 0.012 s / 0.019 s | 45 / 40 MB |
| XML | Format | CSharpier | 4.0× faster | 0.11 s / 0.44 s | 104 / 97 MB |
| Go | Lint | revive | 1.2× slower | 18.07 s / 14.47 s | 2643 / 4115 MB |
| CMake | Format | gersemi | 2.3× slower | 0.69 s / 0.30 s | 356 / 49 MB |
|  | Lint | cmake-lint | 48× faster | 1.45 s / 69.22 s | 557 / 42 MB |
| Kotlin | Format | ktlint | 30× faster | 1.61 s / 48.42 s | 501 / 715 MB |
|  | Lint | ktlint | 42× faster | 1.14 s / 48.01 s | 485 / 683 MB |
| Vue | Lint | eslint-plugin-vue | 73× faster | 0.11 s / 8.27 s | 154 / 634 MB |
| Svelte | Lint | eslint-plugin-svelte | 49× faster | 0.10 s / 4.91 s | 71 / 600 MB |
| HCL | Format | terraform fmt | 3.5× faster | 0.11 s / 0.40 s | 123 / 87 MB |
|  | Lint | tflint | 23× faster | 0.20 s / 4.69 s | 120 / 89 MB |
| Angular | Format | Prettier | 79× faster | 0.096 s / 7.54 s | 83 / 248 MB |
| Ruby | Lint | RuboCop | 88× faster | 3.00 s / 264.55 s | 1180 / 377 MB |
| Protobuf | Format | buf | 3.3× faster | 0.069 s / 0.23 s | 115 / 75 MB |
|  | Lint | buf | 1.3× faster | 0.091 s / 0.12 s | 123 / 39 MB |
| PHP | Format | PHP-CS-Fixer | 161× faster | 1.77 s / 285.46 s | 1161 / 70 MB |
|  | Lint | PHPStan | 84× faster | 1.44 s / 120.93 s | 1194 / 499 MB |

Method, runner and corpora: [docs/performance.md](docs/performance.md).
<!-- /generated -->

Sudsy does the same work as each tool: `format --check` and `lint` with the preset that mirrors it. The binary is about 53 MB: it holds the parsers for every language and a Wasm runtime for plugins.

## Configure

```sh
sudsy init                          # in a terminal: asks about languages, style, tool configs to migrate
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
