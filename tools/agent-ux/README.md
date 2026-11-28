# agent-ux: agent workflow experiments

Runs coding tasks with real headless Claude Code (`claude -p`) under paired conditions (mock tools,
hooks, instruction sentences) and scores them. Method and results:
[docs/ai/design/agent-workflows.md](../../docs/ai/design/agent-workflows.md).

Needs `claude` (logged in), `python3`, `black`, `ruff`, `git`. Not part of the gate or CI; runs cost
money (about $0.005-0.02 per Haiku run, $0.06-0.17 per Sonnet run).

```bash
python3 _gen/mkvariants.py          # writes tools/variants, tasks, extras (generated, not committed)
./run.sh E4 B 0 haiku               # one run: setup + claude -p + score, appended to results.jsonl
./batch.sh haiku 3 6 E1 E3 E7/B     # every condition of E1, E3 and E7/B, 3 reps, 6 in parallel
python3 summary.py results.jsonl    # per condition: turns, tokens, cost, correctness fields
python3 deltas.py results.jsonl     # change against condition A, with the separation rule
```

- `base/`: the fixture repo (package `shop`, tests in `tests/`); edit `_gen/gen_orders.py` for
  `shop/orders.py` and `_gen/mkvariants.py` for task texts and variants, not their outputs.
- `tools/common/`: the mock tool implementations the variants wrap.
- `setup.sh EXP COND REP`: a fresh git-initialised run dir under `runs/`; `score.sh RUNDIR`: one
  JSON line (code in `_score/`); `run.sh`: subjects get a scoped `--allowedTools` list and normal
  permission checks.
- `e12/`: the token count of the minimized-view experiment (no subject).
- `data/`: the measured runs (`haiku.jsonl`, `sonnet.jsonl`; `haiku-v1-invalid.jsonl` is the first
  batch, invalid because of permission denials, kept for the record).
