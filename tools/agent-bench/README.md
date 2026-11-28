# agent-bench: the agent benchmark

Headless Claude Code on pinned real repositories. The main numbers are feature pairs: two Sudsy arms that
differ in one feature (hooks, one check command, the actionable report, `--new`, collapsing, fix ids, an
enforced policy), each on the tasks where it matters; the secondary one compares the native tools with
Sudsy on matched rule sets. Method, pairs and tasks:
[docs/ai/design/agent-integration.md](../../docs/ai/design/agent-integration.md#step-1-the-agent-benchmark-built-no-full-run-yet).
Grown from the pilot in [`tools/agent-ux/`](../agent-ux/README.md) (mock tools, kept as the evidence of
agent-workflows.md).

Needs `claude` (logged in), `git`, `uv`, `node`/`npx`, `go`, and a release build of Sudsy
(`CARGO_TARGET_DIR=<slot> cargo build --release -p cli`; `--sudsy PATH` for another). Costs money: a
smoke batch on Haiku is about $1.5, a full batch on Sonnet well over $100.

```bash
python3 tools/agent-bench/fetch.py                           # clones and tool environments into cache/ (also run by bench.py)
python3 tools/agent-bench/validate.py --sudsy PATH           # every task: fails untouched, passes its reference (no cost)
python3 tools/agent-bench/matched.py --sudsy PATH            # regenerate configs/ (the stack comparison's Sudsy configs)
scripts/agent-bench.sh --smoke --dry-run                     # the plan and the instruction blocks
scripts/agent-bench.sh --smoke [--pairs E1,E3,B]             # a smoke batch -> data/<date>-<sha>-smoke.jsonl
python3 tools/agent-bench/stats.py data/BATCH.jsonl          # per pair: ratios, success, mistakes, size estimate
python3 tools/agent-bench/stats.py --compare A.jsonl B.jsonl # repeatability of two batches
```

- `pairs.toml`: the comparisons; `arms.toml`: what each arm changes (commands, hooks, shim, policy rule,
  config); `repos.toml`: repositories, pins, tool versions, each family's commands; `bench.lock.toml`:
  Claude Code version, models, reps, limits; `configs/`: the matched Sudsy configs of pair B.
- `tasks/v2/<id>/task.toml`: `repo`, `pairs`, `kind`, `text`, `tests`, `scope`, and `commit` + `hidden`
  (tasks from a real commit) or `seed_edits` with `solution_edits[_<family>]` / `solution.patch`, and
  `clean`, `clean_rules`, `formatted`, `project` (rules and config of the project) or `policy` (E10).
- `setup_run.py`, `arms.py` (instruction template `instructions.md`, allowlist, hooks), `hook.py` (the native
  hook arms; the sudsy ones run `sudsy hook`), `wrap.py` (the `sudsy` of the shim arms), `verify.py`, `bench.py` (plan, run, record), `stats.py`.
- Run directories and transcripts: `<slot>/agent-bench/` (`AGENT_BENCH_WORK`), never inside a checkout.
