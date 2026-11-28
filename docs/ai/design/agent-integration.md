# Plan: agent integration (benchmark, output, fixes, new findings, hooks, MCP)

Turns Sudsy into the cheapest checker for a coding agent to run, and measures it in a way that can
be published. Evidence for the choices: [agent-workflows.md](agent-workflows.md) (mock tools,
Haiku and Sonnet; hooks, one command and actionable messages were clear on both models).

What exists today: `sudsy check` (`format --check` + `lint`), `sudsy fix [--unsafe]` (applies fixes
until none is left, then formats, reports what remains), `sudsy rules <id>`, `init --agents` (the
agent block in `AGENTS.md`/`CLAUDE.md`, also a question of the interactive `init`), the LSP over the
incremental database, the pattern-rule engine `core-patterns`. Fix kinds (`core_rules::FixKind`):
`Safe` (applied by `fix`), `Unsafe` (by `fix --unsafe` and editor quick fixes), `Suggestion` (editor
only). The human output prints `path:line:col: severity[rule]: message`, `note:` lines and
`fix (kind): title`, but neither the source line nor the replacement text.

Order: the benchmark first (built; its first full run is the baseline every later step is measured against),
then the steps in the order of their measured effect per cost.

## Step 1: the agent benchmark (built; no full run yet)

The numbers we publish: real tasks on real repositories, the tools people use today as the baseline,
results that repeat within a few percent. Built in [`tools/agent-bench/`](../../../tools/agent-bench/README.md),
run by `scripts/agent-bench.sh [--smoke | --full] [--conditions ...]`. Not in CI: it costs money and
needs a logged-in `claude`. The first full run (the baseline of every later step) waits for the user's
go-ahead; its numbers go to `docs/ai/performance/agent-numbers.md` (latest values only), which that run
creates.

### Repositories and tasks
- **Repositories** (`repos.toml`): click and humanize (Python), ufo (TypeScript), chi (Go), each
  pinned at a commit and cloned blobless by `fetch.py` (git, since codeload tarballs are not reachable
  from every agent host). Each keeps its own tools for the native condition, at the versions of
  `repos.toml` (a venv per Python repository, `node_modules` from the pin's lockfile, the host's Go):
  ruff check + ruff format (click), ruff check + black (humanize), `npm run lint` / `lint:fix`
  (ESLint + Prettier, ufo), `go vet` + `gofmt` (chi).
- **Task set `tasks/v1/`**, 22 tasks (a change to the set is a new version, never compared with the
  old one): 16 bug-fix and feature tasks from real commits (the run starts at the commit's parent,
  the task text is written from its description, the verifier puts the commit's test files in place
  and runs them), 2 refactors with decoys (rename a method that shares its name with others: chi
  `node.findEdge` beside `nodes.findEdge`, click `Context.get_usage` beside `Command.get_usage`),
  2 lint-debt tasks (seeded findings that both families report, in humanize `filesize.py` and ufo
  `utils.ts`), 2 policy tasks ("make it an enforced rule": no `print()` in click's library code, no
  direct `decodeURIComponent` outside ufo's `encoding.ts`). The 9 chi and humanize logic fixes are the
  tasks where Sudsy should not matter.
- **Verifier** (`verify.py`), with the condition's own tools: the task's tests, its checks, its probes
  (policy tasks: a file that must now get a finding and one that must not, against the same file under
  the start config) and its clean files (lint debt: no finding left), nothing changed outside the
  task's scope, no new lint finding on the changed files and no changed file that was formatted at the
  start left unformatted. A finding is new unless the same (path, rule, line text) is in the start tree
  as often. A run succeeds when all pass; the other family's counts are recorded as mistake counters.
- **Validation** (`validate.py`, no subject, no cost): per task and family the setup works, the task's
  own signal fails on the untouched tree and the reference solution (the commit, or
  `solution[.<family>].patch`) passes the whole verifier. All 22 pass.
- Still to write for the plan's 40: the mixed repository (code plus YAML, Markdown, Dockerfile, CI),
  a second TypeScript/JavaScript repository, and about 18 more tasks in the same mix (a ufo feature task
  from 2025 was dropped: its start tree is not clean under the pinned ESLint, and its README example is
  a new Sudsy finding).

### Conditions
One instruction template (`instructions.md`) fills in the tool names: the checks, the fix and format
commands, the tests, the allowed commands, and for the hook conditions one sentence on the hooks.
- `native`: the repository's own commands.
- `native-hooks`: the same tools behind `hook.py`: after each Edit/Write the edited file is linted
  (findings on stderr, exit 2, the pilot's protocol), at Stop the fix commands run on the changed files.
- `sudsy`: `sudsy check` / `sudsy fix` (plus `gofmt` for Go, which Sudsy lints but does not format).
  The start tree has the `sudsy.yaml` of `sudsy init -y` with the presets closest to the native rule set
  (`python/ruff`; `js/eslint-recommended` + `js/typescript-eslint-recommended`; Go defaults) and is
  formatted by `sudsy format`, as a project that uses Sudsy would be. Sudsy does not import ruff or flat
  ESLint configs, so its rule sets differ from the native ones.
- `sudsy-hooks`, recorded as `sudsy-hooks-provisional`: `hook.py` with `sudsy lint FILE` after an edit
  and `sudsy fix` at Stop, until `sudsy hook` (step 5) replaces it.
- `sudsy-mcp` is added with step 6.

### Runs
- `claude -p` with `--output-format stream-json --verbose` (the transcript stays in the batch's work
  directory), `--permission-mode default`, a scoped `--allowedTools` (file tools; the condition's
  checker programs with any arguments, so `sudsy rules` and `ruff rule` are allowed too; its runner
  commands such as `python -m pytest` or `go vet`; read-only shell commands; never a bypass),
  `--setting-sources project`,
  `--strict-mcp-config`, `--no-session-persistence`, a per-run budget cap and timeout
  (`bench.lock.toml`). The allowed commands are listed in the task text; denials are recorded per run.
- Run directories live in the build slot (`AGENT_BENCH_WORK` overrides), outside every checkout, so a
  subject never reads Sudsy's own `CLAUDE.md` (the pilot's runs under `tools/agent-ux/runs/` could).
- Each record has the Claude Code version (checked against `bench.lock.toml`, warned on a mismatch;
  the installed `claude` is used, the pin is not installed), the requested model and the exact model
  ids from the result, the Sudsy binary, version, commit and the tree id of `crates/` (stable when
  tool-only commits are squashed), the task-set version and the costs.
- **Paired and interleaved:** every round runs every task under every condition in a seeded random
  order; `--parallel` runs (default 4) start in plan order. `--max-cost` stops starting runs.
- Smoke: 10 tasks (a seeded sample) x 2 reps on Haiku; full: every task x 5 reps on Sonnet.
- Raw results: `data/<date>-<sudsy sha>[-smoke].jsonl`, one line per run, committed per batch; the
  transcripts of a published batch go out as a release asset.

### Statistics (`stats.py`)
- Per metric (turns, output tokens, uncached, cache-read and cache-creation input tokens, cost, time)
  and condition against `native`: the ratio of geometric means over tasks of the per-task means, with
  a 95% bootstrap interval (2,000 resamples of tasks, and of reps within each task). A ratio whose
  interval lies within ±5% of it is marked publishable; the others are not published.
- Success rate per condition with a Wilson 95% interval, and the mistake counters (unformatted, new
  findings, out of scope, runs with denials).
- `stats.py --compare A B`: the repeatability check; every ratio of each batch must lie in the other's
  interval (exit 1 otherwise). The first published batch runs twice on the same versions.
- Expected size for the bar, from the pilot's spread (20-30% of the mean per metric): about 200
  paired runs per comparison.

## Step 2: actionable output

Built except the message rewording (still open). How it works: [../config/cli.md](../config/cli.md).

- The human output (the one agents read) prints the offending source line under each finding, and
  for a fix its replacement: the edited text for a one-line edit (`fix (safe): replace with
  log.info(...)`), a short `-`/`+` pair for a multi-line one, capped at a few lines.
- `--format concise` for very long reports: the one-line form without source lines (CI logs).
- Rule messages say what to do. The texts of [rule-texts.md](../development/rule-texts.md) keep
  their split (`description` says what is reported); the editor and CLI message of a rule with an
  obvious fix names it ("use `x?.y`"). Done rule by rule, most frequent first by the parity corpora's
  counts; tracked by a list that only shrinks, like `undescribed.txt`.
- A long report collapses by default when it has more than about 100 findings: one line per rule
  with its count and first locations, the full list by `--rule` (E8: input tokens −21% to −38%).

## Step 3: fixes in one call

Built; how it works: [../config/cli.md](../config/cli.md#fix-ids). Two ids can collide (6 base-32 characters: about 1 in 2,000 with 1,000 ids in one report); `--apply` then applies both.

The E3 fix-by-id condition paid one call per fix; applying several in one call removes that.
- Every unsafe and suggestion fix printed gets a short id (a hash of path, rule, the text of the
  finding's line and the fix's index), stable across runs while the line is unchanged. No state is
  stored: `fix` relints and finds the fix by its id.
- `sudsy fix --apply ID[,ID...]` applies exactly those fixes (any kind, suggestions included), then
  formats; an id that no longer exists is reported with the current ones.
- Accept all is `sudsy fix --unsafe` (already there): every safe and unsafe fix. Suggestions stay out
  of accept all: a suggestion is one of several possible changes (the reason it is not unsafe), so
  it is applied only by id.
- The agent instructions say: run `sudsy fix --unsafe` unless a fix must be checked; the report
  after it lists only what needs a decision.

## Step 4: only new findings

### How other tools do it
From documentation known to us at writing time; recheck each before citing it publicly.
- **Baseline files in the repository:** PHPStan (`phpstan-baseline.neon`: message, path and count),
  Psalm (`psalm-baseline.xml`), detekt and ktlint (XML baselines), SwiftLint (`--baseline`), ESLint's
  bulk suppressions (`eslint-suppressions.json`: counts per file and rule). State is a committed file
  that must be regenerated as code changes.
- **Todo configs:** RuboCop `--auto-gen-config` writes `.rubocop_todo.yml` (rules off per file).
- **Inline suppressions:** Ruff `--add-noqa` writes a comment on every current finding.
- **Diff filters outside the linter:** reviewdog (`-filter-mode=added|diff_context`), diff-cover;
  lint-staged lints the staged files whole.

### Design
- `--new[=REV]` on `lint`, `check` and `fix` (default `HEAD`): a finding is new unless the same
  finding exists in the file at REV. No state file: for each changed file Sudsy reads its text at REV
  from git, lints it in memory with the current config, and compares fingerprints (rule, the
  finding's line text without leading whitespace, its occurrence number among equal ones). Line
  shifts do not matter, an edited line's findings count as new. Unchanged files are skipped, so the
  cost is one extra lint per changed file.
- A file not in git, or no repository, makes every finding new (and says so once).
- `--new` prints nothing and exits 0 when no new finding is left, which is what makes the hooks
  silent.
- A committed baseline file (the PHPStan model) is deferred: git covers agents and pull requests;
  it is built when a user needs new-only checks without a usable base revision.
- **Extension:** the LSP keeps showing all findings by default (people edit with the whole file in
  view). A setting `sudsy.diagnostics.scope: all | new` uses the same comparison against `HEAD`, the
  text at `HEAD` cached per file until `HEAD` moves.
- **CI:** `check --new=origin/main` checks a pull request against its base.

## Step 5: hooks

`sudsy hook <event>` reads the hook input JSON of the agent on stdin and answers in its protocol;
Claude Code first (the protocol the experiments used). Recheck the event and exit-code semantics
against the current Claude Code hook documentation when building.
- `sudsy hook post-edit` (PostToolUse, matcher `Edit|Write|MultiEdit`): lints the edited file with
  `--new`, never formats (formatting under the agent makes its next edit miss the text it holds).
  Clean: no output, exit 0. Findings: the step 2 output on stderr, exit 2, so the agent reads them in
  the same turn.
- `sudsy hook stop` (Stop): `fix` (safe fixes) and format on the files changed since `HEAD`
  (tracked and untracked), then `lint --new` on them. Clean: exit 0. Errors left: exit 2 with them,
  so the agent continues; when the input says the stop hook already ran in this stop, exit 0 with
  the findings as a note, so it can never loop.
- `init` writes the settings: the AI agents question (and `--agents`) also merges both hooks into
  `.claude/settings.json` (an existing file is merged with `json_edit.rs`, never overwritten; an
  existing Sudsy hook is left as it is), and the agent block says formatting is automatic.
- Other agents with hooks get their own event names behind the same command once their protocol is
  checked; the instructions-only setup keeps working everywhere.

## Step 6: MCP server

The tools an agent decides to call; the automatic part stays in hooks.
- `sudsy mcp`: an MCP server on stdio over one long-lived database, like the LSP, so repeated calls
  are incremental. A subcommand of the one `sudsy` binary, its code in a crate `crates/mcp` linked
  into the CLI the way `lsp` is (no second binary or release). SDK: measure the `rmcp` crate against a
  small own JSON-RPC loop (binary size, dependencies, startup of the other commands unchanged),
  pick the smaller that covers tools and their input schemas.
- Tools, each returning the step 2 text (compact text costs fewer tokens than JSON):
  - `check(paths, new)`: findings with fix ids, nothing when clean;
  - `fix(paths, ids | all)`: applies, formats, returns what remains;
  - `explain(rule)`: the `sudsy rules <id>` text;
  - `search(pattern, language, paths)`: structural search with the pattern syntax of
    `core-patterns` (also as `sudsy search` on the CLI), matches as `path:line: source line`;
  - `test_rule(rule_yaml, paths)`: compiles a pattern rule and returns its matches without
    writing it, for the "make that an enforced rule" flow (E9).
- **Distribution.** An MCP client launches a command and talks over its stdin and stdout, so every
  existing package serves:
  - installed (npm, PyPI, Homebrew, Scoop, winget, release binary): `{"command": "sudsy", "args":
    ["mcp"]}`, the same version and config as the CLI, hooks and editor;
  - not installed: `npx -y @sudsylint/sudsy@<version> mcp` or `uvx sudsy@<version> mcp` (the npm
    shim passes stdio through to the platform binary), a download on first launch;
  - `init` offers the config for the agents it detects (`.mcp.json` for Claude Code), the same merge
    rules as the hooks: `sudsy mcp` when `sudsy` is on the PATH, else the `npx` form pinned to the
    exact version, never `@latest`, so a release never changes a project's checks unasked;
  - to check before building: an extension API for VS Code to offer the bundled binary as an MCP
    server, and a listing in the public MCP registry.
- Name-aware search (`references` through the symbol index of [symbol-index.md](symbol-index.md),
  the case grep and patterns cannot decide) follows when the index covers the language.

## Measurement gates
- Step 1's first full run on the current release is the baseline (`native` against `sudsy`).
- After each later step: a smoke run of the conditions it changes; the step lands when its own
  metrics improve or stay inside their interval, and others stay inside theirs.
- Before anything is advertised: a full run, twice, on the release that ships the feature.
