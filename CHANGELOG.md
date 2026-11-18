# Changelog

## [Unreleased]

### Lint
* `no-unreachable` is renamed `unreachable-code` (JavaScript/TypeScript config keys and the
  `eslint-recommended` preset follow). It now runs in C#, Java, JavaScript/TypeScript, Kotlin, PHP,
  Python and Ruby, and is on by default (`warn`) in all of them; in C# it was off.
* `no-fallthrough` also runs in Java and PHP.
* JavaScript/TypeScript scope rules are the shared ones and are renamed (config keys and the
  `eslint-recommended` and `typescript-eslint-recommended` presets follow, no aliases):
  `no-undef` is `undefined-variable`, `no-redeclare` is `redeclared-variable`,
  `no-use-before-define` is `use-before-define`, `no-shadow` is `shadowed-variable`, and
  `no-unused-vars` is split into `unused-variable` and the new `unused-parameter`. ESLint's
  options become rule options: `unused-variable` has `caught_errors`, `ignore_pattern` and
  `ignore_rest_siblings`, `unused-parameter` has `args` and `ignore_pattern`, `use-before-define`
  has `functions`, `classes` and `variables`, `shadowed-variable` has `allow` and `kinds`.
* Python scope rules are the shared ones and are renamed (config keys and the `python/ruff` preset
  follow, no aliases): `undefined-name` is `undefined-variable`, `redefined-while-unused` is
  `redeclared-variable`, `undefined-local` (F823) is `use-before-define`, and
  `import-shadowed-by-loop-var` and `redefined-argument-from-local` are merged into
  `shadowed-variable` (so a plain `warn` now reports both; it was `off` for the second).
  Names inside quoted annotations are read as identifiers, not parsed.
* The regex rules are one shared family (`core_rules::regex_source`): JavaScript/TypeScript keeps
  `no-control-regex`, `no-regex-spaces`, `no-empty-character-class`,
  `no-misleading-character-class` and `no-useless-escape` (regex part), and Python has the new
  `no-control-regex`, `no-regex-spaces` and `no-misleading-character-class` (off by default) for
  the constant string given to `re.compile`, `re.match`, `re.search` and the other `re` functions.
  `no-regex-spaces` now also reads the pattern of a string argument with its escapes decoded.

* Rules that check the same thing share one id (config keys and the presets follow, no aliases):
  `no-self-assign` (Ruby `self-assignment`), `boolean-literal-comparison` (Python
  `true-false-comparison`; Java has it as before), `no-wildcard-imports` (Java `avoid-star-import`,
  Python `undefined-local-with-import-star`), `no-useless-expression` (JavaScript
  `no-unused-expressions`, Python `useless-expression`), `no-debugger` (Python `leftover-debugger`),
  `no-print` (Python `print` and `pprint`; a `print(file=...)` to a file and the alias `pp` are no longer told apart), `no-eval` (Python `exec-call`), `no-empty-catch` (Python `try-except-pass`, Ruby
  `suppressed-exception`), `no-broad-catch` (Python `bare-except` and `blind-except`, Ruby
  `rescue-exception`; options `bare` and `types`) and `no-throw-generic` (Python
  `raise-vanilla-class`; option `types`). An empty `catch` is `no-empty-catch`, no longer
  `no-empty-block`. Python `bare-except` and `blind-except` are one rule: `blind-except` is
  `no-broad-catch` with the option `types: [Exception, BaseException]`, and it no longer sees a
  tuple of types. `no-debugger` also reports the Python import of a debugger module.
  CSS `comment-no-empty` is `no-empty-comment` and `comment-whitespace-inside` is `comment-space`
  with the options `block: true` and `line: false` (the stylelint preset sets them).

### Changed
* GraphQL `selection-set-depth`: the option `max_depth` is now `max`, as in every `max` rule.

## [0.0.2] - 2026-11-17

### Fixed
* VS Code and the release archives: JavaScript/TypeScript, CSS, Python, SQL, Java, HTML, Vue,
  Svelte, Angular, Kotlin, PHP and Ruby were reported as not installed and were not formatted or
  linted, and installing them failed with "this build has no `js` language pack". Only
  tree-sitter languages are packs; the others are in the binary.
* Python: the formatter is stable on its own output (comments in lambdas, implicit string
  concatenations, after the head of an attribute chain, in `class C(  # c`).

### Install
* Packages for npm (`npm i -D @sudsylint/sudsy`), PyPI (`pip install sudsy`, `uvx sudsy`), Homebrew
  (`brew install bingsubandit/sudsy/sudsy`), Scoop, winget and `cargo binstall`, built from the
  release archives.
* VS Code: the install prompt for a missing language shows on every open of such a file, with
  "Don't ask again"; the command "Sudsy: Install Missing Languages" installs the languages of the
  open files. `sudsy.importStrategy`: `fromEnvironment` (default) uses `sudsy.serverPath`, then a
  `sudsy` on PATH, then the bundled binary; `useBundled` always uses the bundled one.

### Lint
* On every language with statement roles (C#, Java, JavaScript/TypeScript, Kotlin, PHP, Python,
  Ruby, SQL): `no-self-compare`, `no-constant-condition`, `no-constant-comparison`,
  `no-assignment-in-condition`, `no-empty-block` and `no-duplicate-case`.
* Java, Kotlin and PHP: unused imports, unused local variables, undefined variables (PHP) and
  parameter assignment.
* JavaScript: `no-fallthrough`, `no-unexpected-multiline`, `no-control-regex` and
  `no-misleading-character-class`.
* Vue and Svelte: rules checked against eslint-plugin-vue and eslint-plugin-svelte
  (`no-target-blank` and `no-dupe-on-directives` corrected, fewer false positives), new
  `v-on-event-hyphenation`, `no-v-text-v-html-on-component`, `no-use-v-else-with-v-for`,
  `no-dupe-v-else-if` and `no-unknown-style-directive-property`; on Vue `<script>` and
  `<script setup>`, `require-default-prop`, `require-prop-types` and `no-dupe-keys`, and Svelte
  `no-object-in-text-mustaches`.

### Faster
* SQL: a hand-written parser replaces sqruff (about 40x faster, a twentieth of the memory, no
  512 KB file limit; the binary is 4 MB smaller).
* Startup about 30 percent fewer instructions; HTML, Vue and Svelte formatting cheaper.

## [0.0.1] - 2026-11-17

First release: one formatter and linter, as a CLI and a VS Code extension, configured by
one `sudsy.yaml`. Every layout decision is an option; presets are plain config.

### C# (needs the .NET 10 runtime)
* Formatter with options for brace style, wrapping, blank lines, usings and more; presets
  `csharp/google`, `csharp/csharpier-compat`, `csharp/stylecop`, `csharp/dotnet-runtime` and
  `csharp/unity`. Without a preset the layout is Microsoft's.
* Lint rules on Roslyn binding, types and control flow (unreachable code, unused variables and
  assignments, usings, private members, null dereferences) plus the StyleCop-style layout,
  naming and documentation rules. `analysis: fast|standard|deep` sets how much runs.
* Unity: `csharp/unity` counts `Awake`, `Update`, `OnTriggerEnter` and the other message methods
  as used; `.asmdef`, `.asmref`, `.inputactions` (JSON), `.uss` (CSS) and `.uxml` (XML) are
  formatted. Unity's serialized YAML (`.unity`, `.prefab`, `.asset`, `.meta`) is left alone.

### Data and config files
* JSON and JSONC, YAML, TOML, XML (`.csproj`, `.props`, `.resx`, `.xaml`, ...), INI, `.editorconfig`,
  `.gitignore`: formatting and lint rules; presets `json/prettier-compat`, `json/biome`,
  `yaml/prettier-compat`, `yaml/yamllint`, `toml/taplo-compat`, `xml/csharpier-compat`.
* GitHub Actions workflows and Dockerfiles, Protobuf, HCL, Makefile.

### Web, scripts and docs
* CSS, SCSS and Less, JavaScript and TypeScript (JSX/TSX), HTML with embedded script and style,
  GraphQL, Markdown (embedded code blocks are formatted by their languages).
* Python (ruff's layout and rule families), SQL (sqlfluff's layout and rules; dialects ansi,
  postgres, mysql, tsql), Vue and Svelte single-file components, Angular templates.
* Shell and PowerShell lint.

### Early (formatter or rules incomplete; keep format on save off for these)
* Java: google-java-format layout (84% of a large corpus identical) and Checkstyle-style rules.
* Kotlin: ktlint's whitespace layout without wrapping, and the first ktlint rules.
* C and C++, PHP, Ruby: parsed and linted; files are kept as written by the formatter.

### Tools
* CLI: `format`, `lint`, `fix`, `check`, `rules`, `config print`, `init` (including migration from
  yamllint, Prettier, Biome, ESLint, StyleCop and CSharpier configs); human, JSON, SARIF and GitHub
  Actions output.
* VS Code extension (language server): diagnostics, quick fixes, fix all, format on save, a
  long-lived C# worker with an incrementally updated workspace. The `.vsix` contains the server
  and the worker.
* Rules beyond the built-in ones: ast-grep-style pattern rules (`plugins: patterns:`) and Wasm
  plugins (API v0, **unstable**).
* Packages for linux, macOS and Windows (x64 and arm64).

### Known limits
* The C# workspace is the `.cs` files under the opened roots, not an MSBuild project: package types
  do not resolve, so `remove-unused-usings` stays silent for files with errors.
* `unused-private-member` does not see calls by name (`Invoke("Name")`, `StartCoroutine("Name")`).
* Range formatting is not available.
* The Wasm plugin API changes without notice until v1.
