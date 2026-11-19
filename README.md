<p align="center"><img src="assets/logo/soap_3d.png" alt="Sudsy" width="96"></p>

<h1 align="center">Sudsy</h1>

<p align="center"><b>One formatter and linter for your whole repo.</b><br>
One binary and one config file in place of Prettier, ESLint, ruff, buf, terraform fmt and the rest, measured file by file against each of them.<br>
<b>Ready for 6 languages, early support for 20 more.</b></p>

<p align="center">
<a href="https://github.com/bingsubandit/sudsy/releases"><img alt="release" src="https://img.shields.io/github/v/release/bingsubandit/sudsy"></a>
<a href="https://www.npmjs.com/package/@sudsylint/sudsy"><img alt="npm" src="https://img.shields.io/npm/v/@sudsylint/sudsy"></a>
<a href="https://pypi.org/project/sudsy/"><img alt="PyPI" src="https://img.shields.io/pypi/v/sudsy"></a>
<a href="LICENSE"><img alt="MIT" src="https://img.shields.io/badge/license-MIT-blue"></a>
</p>

> **Early release (v0.0.x).** Ready: JavaScript/TypeScript, Python, Protobuf, Terraform, GraphQL and TOML. The rest are partial; see the [status table](#languages). Config may change in minor versions before 1.0.

## Try it

This is read-only: `check` reports and changes no files. It needs no install and no config.

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

Some languages are small downloads, fetched once with `sudsy languages add` (for example `sudsy languages add toml protobuf`). Until then, their files are skipped with a warning. `sudsy languages list` shows what is installed. The `sudsy-full` release archives contain every language.

## Why

A typical repo runs one tool per language. Each tool has its own install, config file, version pins, editor plugin and CI step. Sudsy is one of each:

```diff
- npx prettier --check . && npx eslint . && ruff format --check && ruff check \
-   && shfmt -d . && buf format -d && terraform fmt -check && markdownlint '**/*.md' \
-   && yamllint . && hadolint Dockerfile
+ sudsy check
```

- **Keep your current style.** A preset reproduces each tool's output. The [table below](#languages) shows how closely, measured on thousands of real-world files per language. Presets are plain config, and every style choice is a named option.
- **Formatting never changes meaning.** Tests and corpus runs check that every formatted file has the same syntax tree as the input, and that formatting twice changes nothing.
- **Your own rules.** Write a rule as a YAML [pattern](docs/rules.md#pattern-rules) (ast-grep style, with fixes), or as a sandboxed [Wasm plugin](docs/rules.md#wasm-plugins).
- **Editor and CI built in.** It includes a language server (with a VS Code extension) and outputs GitHub annotations and SARIF.

**Who it is for:** repos with several languages, where one tool and one config replace five or ten. It fits best today when most of the code is in the ready languages, for example a TypeScript and Python service with Protobuf APIs and Terraform infrastructure. **Who it is not for yet:** a JavaScript-only or Python-only repo already on Biome or ruff. Those tools are faster, and Sudsy adds nothing there except the same config across languages.

## Languages

Each percentage is measured on real-world code against the reference tool's own output, with the tool's default settings.
- **Identical:** the share of files that Sudsy formats byte-for-byte like the reference tool. The main differences per language are in [docs/languages.md](docs/languages.md).
- **Findings:** the share of the reference tool's findings that Sudsy also reports, with the preset named in the table. This is recall only; false positives are not counted here.
- **Preset:** what to put in `extends:` to get that tool's style and rules.

**Ready to format.** At least 85% identical. Lint is ready only where Findings is 85% or more.

| Language | Formats like | Identical | Stable | Lints like | Findings | Presets |
|---|---|---:|---|---|---:|---|
| JavaScript / TypeScript | Prettier | 93% of 2,456 | yes | ESLint, typescript-eslint recommended¹ | 100% | `js/eslint-recommended`, `js/typescript-eslint-recommended` |
| Python | ruff format (Black) | 93% of 1,085 | yes | ruff default rules | 96% | `python/ruff` |
| Protobuf | buf format | 99% of 323 | yes | buf STANDARD | 86% | `protobuf/buf-standard` |
| Terraform (HCL) | terraform fmt | 98% of 318 | yes | tflint recommended | 20% | `hcl/tflint-recommended` |
| GraphQL | Prettier | 98% of 132 | yes | — | — | — |
| TOML | taplo | 95% of 213 | not measured | — | — | `toml/taplo-compat` |

Stable: formatting a second time changes no file in the corpus. ¹ Core rules only. ESLint plugins such as react, import and jsx-a11y, and type-aware TypeScript rules, are not covered yet.

**Partial.** Works, but expect more differences from your current tool. HTML and PowerShell format well but miss the lint bar.

| Language | Formats like | Identical | Lints like | Findings | Preset |
|---|---|---:|---|---:|---|
| HTML | Prettier | 93% of 233 | html-validate | 64% | `html/html-validate-recommended` |
| PowerShell | Invoke-Formatter | 93% of 211 | PSScriptAnalyzer | 34% | `powershell/psscriptanalyzer` |
| PHP | PHP-CS-Fixer (PER-CS) | 88% of 10,382 | PHPStan | 0% | `php/phpstan` |
| Markdown | Prettier | 86% of 64 | markdownlint | 83% | `markdown/prettier-compat`, `markdown/markdownlint` |
| Java | google-java-format | 82% of 5,337 | Checkstyle (Google) | 26% | `java/google` |
| Shell | shfmt | 82% of 82 | ShellCheck | 47% | `shell/shellcheck` |
| SQL | sqlfluff | 78% of 867 | sqlfluff | 4% | `sql/sqlfluff` |
| C# (needs .NET 10) | CSharpier | 72% of 7,615² | StyleCop | not measured | `csharp/csharpier-compat`, `csharp/stylecop` |
| Kotlin | ktlint | 69% of 5,776 | ktlint | 13% | `kotlin/ktlint` |
| Vue / Svelte | Prettier | 83% / 89% | eslint-plugin-vue / -svelte | 87% / 100% | `html/eslint-plugin-vue-recommended`, `…-svelte-recommended` |
| YAML, JSON, CSS, XML | Prettier / Biome / CSharpier | not measured | yamllint / stylelint | 74% / 13% | `yaml/yamllint`, `css/stylelint-standard` |

² The 7,659-file corpus minus the files CSharpier itself leaves alone.

**Lint first:** Ruby is linted, with RuboCop's Lint department at 96% (`ruby/rubocop`), but not reformatted. Dockerfile (hadolint 28%), Makefile (checkmake 74%) and GitHub Actions (actionlint 5%) are formatted too, but no reference formatter has been compared yet.

**Experimental:** C/C++ (not formatted, no rules yet), INI, EditorConfig, .gitignore.

All languages: <!-- generated:languages -->C#, C/C++, CSS, Dockerfile, EditorConfig, Gitignore, GraphQL, HCL, HTML, INI, JSON, Java, JavaScript/TypeScript, Kotlin, Makefile, Markdown, PHP, PowerShell, Protobuf, Python, Ruby, SQL, Shell, TOML, XML and YAML<!-- /generated -->.

Corpora (for example Django for Python, the .NET runtime, Roslyn and ASP.NET Core for C#), tool versions and how to rerun each comparison: [docs/languages.md](docs/languages.md).

## Performance

<!-- generated:speed -->
| Language | Task | Compared with | Sudsy speed | Time (Sudsy / tool) | Peak memory (Sudsy / tool) |
|---|---|---|---|---|---|
| JSON | Format | Prettier | 22× faster | 0.012 s / 0.27 s | 34 / 89 MB |
|  |  | Biome | 4.0× faster | 0.009 s / 0.036 s | 42 / 45 MB |
|  |  | dprint | not measured: no mirroring preset | - | - |
| JS/TS | Format | Prettier | 24× faster | 0.21 s / 5.03 s | 183 / 297 MB |
|  |  | Biome | not measured: no mirroring preset | - | - |
|  | Lint | Biome | not measured: no mirroring preset | - | - |
|  |  | oxlint | not measured: no mirroring preset | - | - |
| Python | Format | ruff | 4.4× slower | 0.35 s / 0.080 s | 239 / 58 MB |
|  | Lint | ruff | 2.4× slower | 0.45 s / 0.19 s | 250 / 57 MB |
| C# | Format | CSharpier | 3.7× faster | 4.39 s / 16.33 s | 558 / 548 MB |

Runner: local run, Linux aarch64, 8 cores (agent host, shared, not the CI runner). Best of 5 runs, a new process each, caches off, `format --check` and `lint` with the Sudsy preset that mirrors the tool. Corpus ([docs/languages.md](docs/languages.md)): JSON: the valid files of JSONTestSuite; JS/TS: three.js src, express lib and test, 8 copies; Python: 1085 files; C#: dotnet/runtime CoreLib. Rerun one row: `scripts/speed.sh <tool>`.
<!-- /generated -->

Sudsy does the same work as each tool: `format --check` and `lint` with the preset that mirrors it. The binary is about 53 MB: it holds the parsers for every language and a Wasm runtime for plugins. Method: [docs/performance.md](docs/performance.md).

## Install

| Channel | Command |
|---|---|
| npm | `npm i -D @sudsylint/sudsy` |
| PyPI | `pip install sudsy` or `uvx sudsy` |
| Homebrew | `brew install bingsubandit/sudsy/sudsy` |
| Scoop | `scoop bucket add sudsy https://github.com/bingsubandit/scoop-sudsy; scoop install sudsy` |
| Binary | [GitHub releases](https://github.com/bingsubandit/sudsy/releases): Linux, macOS and Windows, each for x64 and arm64 |

Only C# needs more. It uses Roslyn in a worker process that every channel above bundles, and the worker needs the [.NET 10 runtime](https://dotnet.microsoft.com/download/dotnet/10.0). Every other language works without .NET.

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
