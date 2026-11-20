# Sudsy

**One formatter and linter for your whole repo.** One binary and one config file in place of Prettier, ESLint, ruff, buf, terraform fmt and the rest, measured file by file against each of them.

<!-- generated:support-headline -->Ready for 8 languages, early support for 19 more.<!-- /generated --> <!-- generated:support-ready -->Ready: JavaScript / TypeScript, Protobuf, GraphQL, TOML, Angular templates, XML, Ruby and Makefile. The rest are in progress.<!-- /generated --> Early release (v0.0.x): config may change before 1.0.

## Try it

Read-only: `check` reports and changes no files. No config needed.

```sh
npx @sudsylint/sudsy check src/      # or: uvx sudsy check src/
```

```
a.py:1:1: error[format]: the file is not formatted; run `sudsy format`
a.py:1:8: warning[unused-import]: `os` imported but unused
b.js:3:18: warning[no-debugger]: Unexpected 'debugger' statement.
```

With no config, lint reports likely bugs only, never style. `sudsy format` rewrites the files; `sudsy fix` applies safe fixes, then formats. Some languages are small downloads: `sudsy languages add toml protobuf`.

## Install

| Channel | Command |
|---|---|
| npm | `npm i -D @sudsylint/sudsy` |
| PyPI | `pip install sudsy` or `uvx sudsy` |
| Homebrew | `brew install bingsubandit/sudsy/sudsy` |
| Scoop | `scoop bucket add sudsy https://github.com/bingsubandit/scoop-sudsy; scoop install sudsy` |
| Binary | [GitHub releases](https://github.com/bingsubandit/sudsy/releases): Linux, macOS, Windows; x64 and arm64 |

Only C# needs more: the [.NET 10 runtime](https://dotnet.microsoft.com/download/dotnet/10.0).

## Configure

```sh
sudsy init --preset python/ruff     # a starter sudsy.yaml
sudsy init --from prettier          # read your .prettierrc and write the matching options
```

## Learn more

- [How close each language is to its reference tool](https://github.com/bingsubandit/sudsy#languages) (Prettier, ruff, buf, terraform fmt, ...)
- [Performance against Prettier, Biome, ruff, oxlint, CSharpier](https://github.com/bingsubandit/sudsy#performance)
- [Configuration](https://github.com/bingsubandit/sudsy/blob/main/docs/configuration.md), [rules and plugins](https://github.com/bingsubandit/sudsy/blob/main/docs/rules.md), [editors and CI](https://github.com/bingsubandit/sudsy/blob/main/docs/editors-and-ci.md)
- [Source, issues and changelog](https://github.com/bingsubandit/sudsy)

MIT licensed.
