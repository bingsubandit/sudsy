# Rules

Sudsy has three kinds of lint rules. All three report into the same diagnostics and fixes, and all three are configured under `lint.rules`.

| Kind | Written in | Needs a build | Use it for |
|---|---|---|---|
| [Built-in rules](#built-in-rules) | Rust and YAML, shipped in the binary | no | The rules that come with Sudsy. |
| [Pattern rules](#pattern-rules) | YAML | no | Team rules that match code shapes, with fixes. |
| [Wasm plugins](#wasm-plugins) | Rust compiled to Wasm | yes | Rules that need real logic. |

## Built-in rules

```sh
sudsy rules                       # list every rule
sudsy rules --language python     # only rules for one language, shared ones included
sudsy rules unused-import         # explain one rule by id
sudsy rules SA1005                # an id from another tool shows the Sudsy rule that covers it
sudsy rules truthy                # other text searches ids, descriptions and other tools' ids
```

`sudsy rules <id>` shows the languages, category, default level, fix kind, options, the presets that enable the rule, and the equivalent rules of other tools. It exits with 2 when nothing matches.

Full list with examples: [ai/rules/builtin.md](ai/rules/builtin.md).

### Severity

Each rule has one of three levels: `off`, `warn` or `error`. Set it in `sudsy.yaml`:

```yaml
python:
  lint:
    rules:
      unused-import: error
      max-line-length: { level: warn, options: { max: 100 } }
```

`check` and `lint` exit with 1 when any diagnostic has level `error`. Warnings do not change the exit code. See [configuration](configuration.md#lint-rules).

### Default policy: bugs only

With no config, a rule is on only when it catches one of these:

- a likely bug
- code that does not do what it says
- invalid or broken output
- a security problem
- deprecated or removed syntax

Level `error` is for code that cannot work, such as an assignment to a `const`. Everything else is `warn`.

Style, naming, ordering, layout owned by the formatter, opinions and noisy rules are off. A repository without config therefore sees few findings. To get a tool's whole rule set, extend its preset, for example `preset:python/ruff` or `preset:shell/shellcheck`. See [presets](configuration.md#presets).

### Fixes

| Kind | What it does | How to apply |
|---|---|---|
| Safe | Keeps behavior. | `sudsy fix` |
| Unsafe | Can change behavior or needs a newer language version. | `sudsy fix --unsafe`, `sudsy fix --apply <id>`, or the editor quick fix titled "(unsafe)" |
| Suggestion | One of several possible changes. | Editor quick fix, or `sudsy fix --apply <id>` |

`sudsy fix` applies fixes in rounds, at most 10. After each round it parses the file again. If the file no longer parses, it undoes that round and names the rules in a warning. Then it formats the file. Fixes that overlap wait for the next round.

### Turn a rule off in code

```js
// sudsy-disable-next-line no-debugger
debugger;
```

See [suppression comments](configuration.md#suppression-comments).

## Pattern rules

A pattern rule is a YAML file. The syntax follows ast-grep. Patterns are snippets of the target language, parsed by that language's own parser. No Rust build is needed.

Enable the files in `sudsy.yaml`:

```yaml
plugins:
  patterns: ["rules/*.yaml"]    # globs relative to this file
```

A complete rule, `rules/no-print.yaml`:

```yaml
id: team/no-print                  # plain id, or team/<name>
language: python
message: Use logging, not print($$$ARGS)
severity: warn                     # off | warn | error: the level when config is silent
category: suspicious               # correctness | suspicious | style | cleanup | performance
rule:
  pattern: print($$$ARGS)
fix: logging.info($$$ARGS)
fix-kind: unsafe                   # safe | unsafe (default) | suggestion
```

Run `sudsy lint a.py`. The rule reports on every `print(...)` call. `sudsy fix --unsafe` rewrites the call to `logging.info(...)`.

Pattern rules show up in `sudsy rules`, take levels under `lint.rules`, honor suppression comments, and appear in the editor like built-ins.

### Main keys

| Key | Meaning |
|---|---|
| `id`, `language`, `message` | Required. `language` is the grammar name, for example `python`, `csharp`, `json`, `yaml`. |
| `rule` | What to match. See below. |
| `constraints` | Conditions on captured names: `regex`, `not-regex`, `kind`. |
| `fix`, `fix-kind`, `fix-title` | Replacement text, its safety, its menu title. `fix: ""` deletes. |
| `report-on` | Where the diagnostic points: `$NAME`, `first-token`, `start`. Default: the matched node. |
| `options` | Typed user options: `bool`, `int`, `string`, `list`, `regex`. Set under `lint.rules`. |
| `description`, `why`, `example` | Shown by `sudsy rules <id>`: what the rule reports, why it matters, an example. |

Unknown keys are errors.

### Inside `rule`

- `pattern`: a code snippet. `$X` matches one node. `$$$ARGS` matches any run of nodes. The same name twice must match equal text.
- `kind`: a node kind of the language's grammar.
- `regex`: a regular expression on the node's text.
- `inside`, `not-inside`, `has`, `not-has`, `parent`, `child`: relations to other nodes.
- `all`, `any`, `not`: combine rules.
- A rule must start from a `pattern` or a `kind`.

Messages and fixes can use `$NAME` and `${NAME}`. A rule file can hold several rules separated by `---`.

A rule that matches too much stops after 1,000,000 steps per node and is off for the rest of that file.

Reference: [ai/rules/patterns.md](ai/rules/patterns.md).

## Wasm plugins

A plugin is a rule written in Rust and compiled to a WebAssembly component. It runs inside Sudsy in a sandbox.

### Do you need a plugin?

Try [pattern rules](#pattern-rules) first. Use a plugin only when a rule needs code: flow analysis, counting, or logic across several nodes or files.

### Status and where the SDK lives

- **The plugin API is unstable before 1.0.** It can change without notice. A plugin built for another API version is refused with a message to rebuild it.
- The interface is a WIT package, `sudsy:rules@0.2.0`. That number names the package version. It does not promise stability.
- The Rust SDK, `plugin-sdk`, is not on crates.io. Use it from a clone of the Sudsy repository (`crates/plugin-sdk`) as a path dependency.
- A plugin in another language needs only the WIT file `wit/rules.wit` from the repository.

### Sandbox limits

- No files, network, environment variables or stdio.
- Per call: about 10 seconds of work and 10 seconds of wall time.
- Per plugin instance: 64 MiB of memory.
- Any trap, exhausted limit or invalid result disables the whole plugin for that run. Sudsy reports one diagnostic at the file start: "plugin `x` is disabled for this run".
- Plugin `.wasm` files over 32 MiB are refused.

### A sketch

`Cargo.toml` has `crate-type = ["cdylib"]` and `plugin-sdk = { path = "<clone>/crates/plugin-sdk" }`. The rule implements `Rule` and is exported by name:

```rust
use plugin_sdk::{Rule, RuleContext, RuleMeta, SyntaxKind, SyntaxNode, kind_of};

pub struct NoDebugFlag;
static META: RuleMeta = RuleMeta { id: "no-debug-flag" /* category, description, default_level ... */ };

impl Rule for NoDebugFlag {
    fn meta(&self) -> &'static RuleMeta { &META }
    fn kinds(&self) -> &[SyntaxKind] { /* [kind_of("json", "Member")] */ }
    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        // inspect `node`, then report a range with a message
        // ctx.report(range, "debug is on".to_string());
    }
}

plugin_sdk::export_plugin!(name: "team", rules: [("json", &NoDebugFlag)]);
```

The rule id is `<plugin name>/<rule id>`: here `team/no-debug-flag`. A rule's language is `json`, `yaml`, `csharp` or `*` for all.

The full sample, with options, fixes, flow-graph rules and project rules, is `tests/plugins/sample` in the repository. More: [ai/rules/plugins.md](ai/rules/plugins.md).

### Build and enable

```sh
rustup target add wasm32-wasip2
cargo build --release --target wasm32-wasip2
```

```yaml
# sudsy.yaml
plugins:
  wasm: ["plugins/team_rules.wasm"]
json:
  lint:
    rules:
      team/no-debug-flag: error
```

Compiled code is cached on disk. Set `SUDSY_WASM_CACHE` to move the cache, or to `off` to disable it. Do not share that cache with untrusted CI jobs. It holds native code.
