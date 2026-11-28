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

Order: the benchmark first (its first run is the baseline every later step is measured against),
then the steps in the order of their measured effect per cost.

## Step 1: the agent benchmark

The numbers we publish. Requirements: real tasks on real repositories, a fair comparison with the
tools people use today, results that move by less than a few percent between two runs of the same
version, and a setup anyone can rerun from the repository.

### Tasks
- **Repositories:** 6-8 open-source projects pinned at a commit, fetched like the parity corpora
  (`scripts/fetch-*`), covering the languages agents edit most: TypeScript/JavaScript (2), Python
  (2), one Go or Java project, one mixed repository (code plus YAML, Markdown, Dockerfile, CI). Each
  keeps its own lint and format setup for the native condition.
- **40 tasks**, a fixed, versioned set (`tasks/v1/`); a change to the set is a new version and its
  numbers are never compared with the old one's. Kinds, in proportions close to agent work:
  - feature and bug-fix tasks from real commits: the commit is reverted, the task text is written
    from its description, its tests (and the repository's) verify;
  - refactors with decoys (rename a method that shares its name with others);
  - lint-debt tasks: "make the checks pass in module X" on a file with real findings;
  - policy tasks: "never do X here; make it an enforced rule" (custom rules are the product's
    claim);
  - tasks where Sudsy should not matter (a pure logic change), so the set does not flatter it.
- **Verifier per task:** the tests, the checks clean on the changed files, the changed files
  formatted, nothing changed outside the task's files, plus the task's own checks. A run counts as
  a success only when all pass.

### Conditions
Every condition gets the same task text and one generated instruction block (same template, only
the tool names differ), so wording is not the variable.
- `native`: the repository's own tools (for example ESLint + Prettier + tsc, or ruff + black +
  mypy), as the project scripts call them.
- `native-hooks`: the same tools behind a hook script. This separates "hooks help" from "Sudsy
  helps"; a claim that only hooks earn is not a Sudsy claim.
- `sudsy`: `sudsy check` / `sudsy fix` in the instructions, no hooks.
- `sudsy-hooks`: the hooks of step 5 as `init` writes them.
- `sudsy-mcp` (after step 6): the MCP server instead of the CLI.

### Runs
- Headless Claude Code (`claude -p --output-format json`) at a pinned version (installed from the
  bench's own lockfile), exact model ids (Sonnet as the published model, Haiku for smoke runs),
  normal permission checks with a scoped allowlist per condition, a fresh worktree per run, tool
  versions pinned by the bench's lockfiles. The task ends with the allowed commands, so permission
  denials stay near zero (a run with denials is reported, never hidden).
- **Paired and interleaved:** every round runs every task under every condition, in a random order,
  so drift of the API or the model over a day hits all conditions alike. Conditions are compared
  only within one batch; a new Sudsy version reruns `native` in the same batch.
- **Reps:** 5 per task and condition for a published run (40 × 5 × 5 conditions = 1,000 runs, about
  $100-150 on Sonnet at the measured $0.06-0.17 per run); a smoke run is 10 tasks × 2 reps × the
  conditions a change touches, on Haiku (about $1).

### Statistics
- Per task and metric, the mean over reps; the change between two conditions is the ratio of their
  geometric means over tasks, with a 95% bootstrap interval that resamples tasks (and reps within
  them). Success rate with a Wilson interval.
- **Stability target:** a published number has an interval of at most ±5%. With the per-run spread
  seen so far (about 20-30% of the mean per metric), 200 paired runs per comparison give about ±4%;
  the report says when a comparison misses the target, and that number is not published.
- **Repeatability check:** the first published batch runs twice on the same versions; the two
  results must agree within their intervals before anything is advertised.
- Metrics: turns, output tokens, input tokens (uncached and cached separately), cost, wall time,
  success rate, and the mistake counters of the verifier (unformatted at the end, files outside
  scope, checks failing).

### Outputs
- `tools/agent-bench/` (grown from `tools/agent-ux/`: setup, run, score, stats);
  `scripts/agent-bench.sh [--smoke | --full] [--conditions ...]`.
- Raw results committed per batch (`data/<date>-<sudsy sha>.jsonl`, with Claude Code version, model
  ids, task-set version and costs); transcripts archived as a release asset, so every published
  number can be traced to its runs.
- `docs/ai/performance/agent-numbers.md`: the latest published numbers with intervals (latest values
  only); the public page reads the same numbers.
- Not in CI: it costs money and needs a login. A hand-started workflow with an API key secret is a
  later option.

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
