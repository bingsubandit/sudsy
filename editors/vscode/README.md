# Sudsy for VS Code

[Sudsy](https://github.com/bingsubandit/sudsy) is one formatter and linter for your whole repo. This extension formats and lints <!-- generated:languages -->HTML, JavaScript/TypeScript, CSS, JSON, Markdown, Python, YAML, Shell, Dockerfile, SQL, Java, C#, PowerShell, Makefile, TOML, XML, Go, CMake, Kotlin, HCL, EditorConfig, Gitignore, INI, Ruby, GraphQL, Protobuf, C/C++ and PHP<!-- /generated -->: diagnostics, quick fixes, fix all
and format on save, from the `sudsy.yaml` of your repository. How close each language is to its usual tool: [status table](https://github.com/bingsubandit/sudsy#languages).

## Install

Install `sudsy.sudsy` from the [VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=sudsy.sudsy), or from [Open VSX](https://open-vsx.org/extension/sudsy/sudsy) for Cursor, Windsurf and VSCodium:

```sh
code --install-extension sudsy.sudsy
```

The fallback is the `.vsix` for your platform from the [latest release](https://github.com/bingsubandit/sudsy/releases): `code --install-extension sudsy-<platform>-<version>.vsix`.

It bundles the `sudsy` server. C# needs the [.NET 10 runtime](https://dotnet.microsoft.com/download/dotnet/10.0); every other language works without it.

## Format on save

Choose Sudsy as the formatter of each language in your settings:

```json
{
  "[typescript]": { "editor.defaultFormatter": "sudsy.sudsy", "editor.formatOnSave": true },
  "[python]": { "editor.defaultFormatter": "sudsy.sudsy", "editor.formatOnSave": true },
  "[yaml]": { "editor.defaultFormatter": "sudsy.sudsy", "editor.formatOnSave": true },
  "editor.codeActionsOnSave": { "source.fixAll.sudsy": "explicit" }
}
```

`sudsy.yaml` itself gets completion and validation (through the Red Hat YAML extension, if you have it installed).

## Settings

* `sudsy.importStrategy`: `fromEnvironment` (default: `sudsy.serverPath` if set, else the first `sudsy` on PATH, else the bundled one) or `useBundled`.
* `sudsy.serverPath`: the `sudsy` executable. A CLI for terminals is a separate install (`npm i -D @sudsylint/sudsy`, `pip install sudsy`, Homebrew, Scoop or the release archives).
* `sudsy.csharpWorkerPath`: `Sudsy.Worker.dll` (default: the one beside the executable).
* Both path settings are machine-scoped: set them in user settings, not in a workspace's `.vscode/settings.json`. The extension does not run in untrusted (Restricted Mode) workspaces.
* `sudsy.trace.server`: `off`, `messages` or `verbose`.

## Building

`npm ci`, then `npm run lint`, `npm test` and `npm run build`. `npm run stage-server -- <release
sudsy> <published worker directory>` copies the native binary and the worker into `server/`, and
`npm run package -- --target <platform>` makes the `.vsix` of that platform (see
`.github/workflows/ci.yml`).
