# Editors and CI

## VS Code

Install `sudsy.sudsy` from the [VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=sudsy.sudsy), or from [Open VSX](https://open-vsx.org/extension/sudsy/sudsy) for Cursor, Windsurf and VSCodium:

```sh
code --install-extension sudsy.sudsy
```

The fallback is the `.vsix` for your platform from the [GitHub release](https://github.com/bingsubandit/sudsy/releases) (`code --install-extension sudsy-<platform>-<version>.vsix`). Platforms: `linux-x64`, `linux-arm64`, `darwin-x64`, `darwin-arm64`, `win32-x64`, `win32-arm64`. The package bundles `sudsy` and the C# worker. It starts `sudsy lsp` and adds nothing else. It covers all supported languages, all built in.

Enable format on save per language, and fix on save:

```jsonc
// .vscode/settings.json
{
  "[python]": { "editor.defaultFormatter": "sudsy.sudsy" },
  "[javascript]": { "editor.defaultFormatter": "sudsy.sudsy" },
  "editor.formatOnSave": true,
  "editor.codeActionsOnSave": { "source.fixAll.sudsy": "explicit" }
}
```

The extension sets no defaults.

| Setting | Meaning |
|---|---|
| `sudsy.importStrategy` | `fromEnvironment` (default): use `sudsy.serverPath`, else `sudsy` on PATH, else the bundled binary. `useBundled`: always the bundled binary. |
| `sudsy.serverPath` | Path to a `sudsy` binary. User settings only. |
| `sudsy.csharpWorkerPath` | Path to the C# worker. Default: beside the executable. User settings only. |
| `sudsy.trace.server` | Protocol trace in the Sudsy output channel. |
| `sudsy.diagnostics.scope` | `all` (default): every finding. `new`: only findings the file at git `HEAD` does not have. |

- A project can provide its own binary with `npm i -D @sudsylint/sudsy`. For a `sudsy` in a terminal, install the CLI separately.
- The two path settings cannot be set in a workspace `.vscode/settings.json`. This stops a repository from choosing which program runs.
- The extension does not run in Restricted Mode. It starts a program and reads repository config, including plugins.
- It validates `sudsy.yaml` against the JSON Schema. Install `redhat.vscode-yaml` to see the hints.
- Palette commands: restart server, show output.
- Every diagnostic has the quick fix "Show documentation for <rule>".

## Other editors

`sudsy lsp` is a language server over stdio. Point any LSP client at it.

```sh
sudsy lsp
```

What it supports:

- Diagnostics by pull (`textDocument/diagnostic`). A client without pull support gets none.
- Quick fixes for each fix. Unsafe fixes are marked "(unsafe)".
- "Fix all `<rule>` problems in this file" and `source.fixAll.sudsy` for fix on save.
- Whole-document formatting. Range formatting is not offered.
- Config reload when `sudsy.yaml` or `.editorconfig` changes.
- Only `file:` URIs. Positions are UTF-16.

The server uses the config of the first workspace folder. For C#, a missing .NET 10 runtime shows one message per session. Other languages keep working.

## GitHub Actions

```yaml
# .github/workflows/sudsy.yml
name: sudsy
on: [push, pull_request]
jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with: { node-version: 22 }
      - run: npx @sudsylint/sudsy check --format github
```

Or use the action of this repository (composite, installs `@sudsylint/sudsy` from npm, which the runner's Node provides):

```yaml
      - uses: actions/checkout@v4
      - uses: bingsubandit/sudsy@v0
        with:
          version: latest        # or a release such as 0.1.0; `local` uses the sudsy already on PATH
          args: src              # optional, after `sudsy check --format github`
          working-directory: .   # optional
```

`--format github` prints one annotation per finding. The step fails on any error or unformatted file. If the repo has C# files, add `actions/setup-dotnet` with `dotnet-version: 10.0.x`.

### Code scanning (SARIF)

```yaml
      - run: npx @sudsylint/sudsy check --format sarif > sudsy.sarif
        continue-on-error: true
      - uses: github/codeql-action/upload-sarif@v3
        with: { sarif_file: sudsy.sarif }
```

SARIF is version 2.1.0. It lists results and the rule table. It does not carry fixes.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Success. |
| 1 | An unformatted file or a lint error. |
| 2 | A file could not be processed, or the config is invalid. |

## Other output

The default output prints each finding with its source line and each fix with the line it produces (or a short `-`/`+` pair). A fix that `sudsy fix` does not apply by itself has an id: `fix k3m9qa (unsafe): ...`. `sudsy fix --apply k3m9qa,7tq2xw` applies exactly those fixes, suggestions included, then formats; `sudsy fix --unsafe` applies every safe and unsafe fix. Ids are hashes of the path, rule and line text, so they hold while the line is unchanged. A report of more than 100 findings is one line per rule; `--rule <id>` lists one rule in full and `--format concise` prints one line per finding.

`--new` on `lint`, `check` and `fix` reports only findings the file did not have at the last commit (`--new=origin/main`: at that revision). Moved lines keep their findings old; exit code and summary count only the new ones. It needs a git repository.

`--format json` gives `{version, summary, diagnostics}` with byte offsets, line and column ranges and fix edits. Use it in scripts. The [site](https://bingsubandit.github.io/sudsy/cli/) documents the shape, and publishes `llms.txt` and a Markdown copy of every page for coding agents.

## Coding agents

`sudsy init --agents` adds a short block to `AGENTS.md` (or to `CLAUDE.md` when only that exists) that tells an agent to run `sudsy check`, `sudsy fix --unsafe` and `sudsy fix --apply <id>`, and merges two Claude Code hooks into `.claude/settings.json`: after each edit `sudsy hook post-edit` shows the agent the new findings of that file, and when it stops `sudsy hook stop` fixes and formats the changed files and turns it back once if findings are left. It writes nothing without the flag and a second run changes nothing.

## pre-commit

The repository provides the hooks `sudsy-check` (`sudsy check`) and `sudsy-fix` (`sudsy fix`). They use `language: system`, so `sudsy` must be on PATH (`npm i -g @sudsylint/sudsy`, `pip install sudsy`, brew, scoop):

```yaml
# .pre-commit-config.yaml
repos:
  - repo: https://github.com/bingsubandit/sudsy
    rev: v0.1.0   # a release tag
    hooks:
      - id: sudsy-check
```
