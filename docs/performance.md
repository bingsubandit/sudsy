# Performance

Speed against the tools Sudsy replaces: the generated table in the [README](../README.md#performance), from `bench/speed.json`.

## Method

- `scripts/speed.sh [<tool>...]` measures every tool with a `speed` key in `scripts/parity_tools.json` (key format: [ai/development/parity.md](ai/development/parity.md#speed)). A new tool needs no script change.
- Same work on both sides: `format --check` and `lint` with the Sudsy preset that mirrors the tool; the tool runs its check command on a folder.
- A new process per run, caches off. Sudsy and the tool alternate; best of 5 wall seconds, and the peak memory of the best run.
- The numbers come from one `speed.yml` run on `ubuntu-24.04-arm`; compare ratios, not absolute values.

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
