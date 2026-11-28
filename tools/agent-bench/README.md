# agent-bench: the agent benchmark

Headless Claude Code on pinned real repositories, paired conditions (native tools, native behind hooks,
Sudsy, Sudsy behind hooks), a verifier per task, geometric-mean ratios with bootstrap intervals. Method,
task set and conditions: [docs/ai/design/agent-integration.md](../../docs/ai/design/agent-integration.md#step-1-the-agent-benchmark-built-no-full-run-yet).
Grown from the pilot in [`tools/agent-ux/`](../agent-ux/README.md) (mock tools, kept as the evidence of
agent-workflows.md).

Needs `claude` (logged in), `git`, `uv`, `node`/`npx`, `go`, and a release build of Sudsy
(`CARGO_TARGET_DIR=<slot> cargo build --release -p cli`; `--sudsy PATH` for another). Costs money: a
smoke batch on Haiku is about $1, a full batch on Sonnet $100-150.

```bash
python3 tools/agent-bench/fetch.py                         # clones and tool environments into cache/ (also run by bench.py)
python3 tools/agent-bench/validate.py --sudsy PATH         # every task: fails untouched, passes its reference (no cost)
scripts/agent-bench.sh --smoke --dry-run                   # the plan and the instruction blocks
scripts/agent-bench.sh --smoke [--conditions native,sudsy] # a smoke batch -> data/<date>-<sha>-smoke.jsonl
python3 tools/agent-bench/stats.py data/BATCH.jsonl        # ratios against native, success, mistakes
python3 tools/agent-bench/stats.py --compare A.jsonl B.jsonl  # repeatability of two batches
```

- `repos.toml`: repositories, pins, tool versions, the commands of each family; `bench.lock.toml`: the
  Claude Code version, models, reps, limits.
- `tasks/v1/<id>/task.toml`: `repo`, `kind`, `text`, `tests`, `scope`, and `commit` + `hidden` (tasks
  from a real commit) or `seed.patch` + `solution[.native|.sudsy].patch` with `checks`, `probes` or
  `clean` (seeded tasks).
- `setup_run.py`, `conditions.py` (instruction template `instructions.md`, allowlist, hooks), `hook.py`,
  `verify.py`, `bench.py` (plan, run, record), `stats.py`.
- Run directories and transcripts: `<slot>/agent-bench/` (`AGENT_BENCH_WORK`), never inside a checkout.
