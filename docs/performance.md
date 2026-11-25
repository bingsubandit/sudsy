# Performance

Speed against the tools Sudsy replaces: the generated table in the [README](../README.md#performance), from `bench/speed.json`.

## Method

- `scripts/speed.sh [<tool>...]` measures every tool with a `speed` key in `scripts/parity_tools.json` (key format: [ai/development/parity.md](ai/development/parity.md#speed)). A new tool needs no script change.
- Same work on both sides: `format --check` and `lint` with the Sudsy preset that mirrors the tool; the tool runs its check command on a folder.
- A new process per run, caches off. Sudsy and the tool alternate; best of 5 wall seconds, and the peak memory of the best run.
- The numbers come from one `speed.yml` run on `ubuntu-24.04-arm`; compare ratios, not absolute values.

<!-- generated:speed-method -->
Runner: Linux aarch64, 8 cores. Best of 5 runs, a new process each, caches off, `format --check` and `lint` with the Sudsy preset that mirrors the tool. Rerun one row: `scripts/speed.sh <tool>`.

Corpus of each row ([docs/languages.md](languages.md) has the sources):

- HTML: the HTML corpus (mdn learning-area and others), without 37 files Sudsy cannot parse and the HTML corpus (Prettier tests, mdn learning-area)
- JS/TS: three.js src, express lib and test, 8 copies and TypeScript sources of vuejs/core, docusaurus, excalidraw and MUI
- CSS: the CSS, SCSS and Less corpus and the CSS corpus (css-tricks, Bootstrap, and others)
- SCSS: the SCSS files of the CSS corpus (Bootstrap and others)
- Less: the Less files of the CSS corpus
- JSON: the valid files of JSONTestSuite and the JSON files of SchemaStore and the VS Code extension samples
- Markdown: the dotnet/runtime documentation
- Python: the Python corpus (Django, Requests, Flask, Click, HTTPX, pytest, Black)
- YAML: the GitHub Actions workflows of real repositories, 8 copies, without 16 files Sudsy cannot parse and the GitHub Actions workflows of real repositories and the yaml-test-suite cases
- Shell: the shell script corpus
- Dockerfile: the Dockerfile corpus
- SQL: the ANSI dialect fixtures of sqlfluff, without 3 files Sudsy cannot parse
- Java: the Java corpus (5 projects)
- GitHub Actions: the GitHub Actions workflows of real repositories
- C#: dotnet/runtime CoreLib, without 14 files Sudsy cannot parse
- C# (.editorconfig): dotnet/runtime CoreLib, without 14 files Sudsy cannot parse
- PowerShell: 211 files, without 1 files Sudsy cannot parse and 210 files
- Makefile: the Makefile corpus
- TOML: the toml-test valid files and the TOML fixtures of the repository
- XML: the XML fixtures and the project files of dotnet/aspnetcore, without 1 files Sudsy cannot parse
- Go: the Go corpus (Go, Kubernetes, Hugo, Prometheus)
- CMake: the CMake corpus (LLVM, CMake, Qt, VTK, OpenCV and others), without 3 files Sudsy cannot parse and the same corpus with all 9066 files
- Kotlin: the Kotlin corpus
- Vue: the .vue files of Vue core, VitePress, Pinia and Nuxt UI
- Svelte: the .svelte files of SvelteKit and the RealWorld app
- HCL: the Terraform corpus
- Angular: 453 files, without 7 files Sudsy cannot parse
- Ruby: the Ruby corpus
- Protobuf: the Protobuf corpus and the Protobuf corpus, one module per project
- PHP: the PHP corpus and 10390 files, without 1 files Sudsy cannot parse
<!-- /generated -->

## C# lint

Lint uses Roslyn's semantic model, one file at a time with one worker.

| Tree | Files | `fast` | `standard` | `deep` | `standard` median / p95 per file |
|---|---:|---:|---:|---:|---|
| CoreLib | 1,679 | 5.1 s | 56.1 s | 57.1 s | 16 / 110 ms |
| Roslyn | 2,858 | 6.1 s | 77.3 s | 82.6 s | 11 / 72 ms |
| aspnetcore | 3,122 | 4.5 s | 74.1 s | 85.4 s | 12 / 42 ms |

`standard` is 10 to 15 times slower than `fast`. This is the slowest part of Sudsy. Use `--analysis fast` for quick runs.

## Other languages

Not compared against other tools on wall-clock time: PHP, Java, Kotlin, Ruby, SQL, HTML, CSS, Markdown, Shell, HCL, Protobuf, TOML, XML. Sudsy tracks instruction counts for these to catch regressions. They are not a comparison with other tools.

## Binary size

The stripped Linux release binary is about 53 MB (52,990,712 bytes, measured with `scripts/binsize.sh`). It holds the parsers of every language and a Wasm runtime (wasmtime, about 20 MB) for plugins. C# also needs the bundled worker directory and the .NET 10 runtime. Size of the current release: not measured.

## Rerun

| What (needs a source checkout) | Command |
|---|---|
| One speed row, or all | `scripts/speed.sh [<tool>...]` |
| Binary size | `scripts/binsize.sh` |
| Instruction counts of a change | `scripts/bench.sh` |

Details: [ai/performance/numbers.md](ai/performance/numbers.md), [ai/performance/tooling.md](ai/performance/tooling.md).
