#!/usr/bin/env python3
"""summary.py [results.jsonl]: per exp/cond mean [each rep] of metrics and key correctness fields."""
import collections, json, pathlib, statistics, sys

path = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else pathlib.Path(__file__).resolve().parent / "results.jsonl"
KEY = {"E1": ["task_tests_pass", "formatted", "ruff_new_findings", "diff_deletions"], "E11": ["task_tests_pass", "formatted", "ruff_new_findings", "diff_deletions"],
       "E2": ["task_tests_pass", "new_findings", "preexisting_findings_fixed"], "E3": ["lint_clean", "tests_pass"],
       "E4": ["rename_correct", "real_renamed", "decoys_broken"], "E5": ["task_tests_pass", "uses_other_rounding"],
       "E6": ["lint_clean", "task_tests_pass"], "E7": ["lint_clean", "tests_pass"],
       "E8": ["r2_r3_clean", "r1_scope_creep", "tests_pass"], "E9": ["rule_ok", "false_positives_in_clock"], "E10": ["task_tests_pass", "uses_get_many"]}
METRICS = [("num_turns", "turns"), ("output_tokens", "out_tok"), ("total_cost_usd", "cost$"), ("duration_ms", "dur_s")]
rows = collections.defaultdict(list)
for ln in path.read_text().splitlines():
    if ln.strip():
        d = json.loads(ln)
        rows[(d["exp"], d["cond"])].append(d)


def fmt(vals):
    vals = [v for v in vals if v is not None]
    if not vals:
        return "-"
    return f"{statistics.mean(vals):.4g} [{', '.join(f'{v:.4g}' for v in vals)}]"


def key(vals):
    vals = [len(v) if isinstance(v, list) else v for v in vals if v is not None]
    if vals and all(isinstance(v, bool) for v in vals):
        return f"{sum(vals)}/{len(vals)}"
    if vals and all(isinstance(v, (int, float)) for v in vals):
        return fmt([float(v) for v in vals])
    return str(vals)


for (exp, cond), rs in sorted(rows.items(), key=lambda kv: (int(kv[0][0][1:]), kv[0][1])):
    print(f"== {exp}/{cond}  n={len(rs)}")
    for k, label in METRICS:
        print(f"   {label:<8} {fmt([(None if r.get(k) is None else r[k] / 1000 if k == 'duration_ms' else r[k]) for r in rs])}")
    for k in KEY.get(exp, []):
        print(f"   {k:<26} {key([r.get(k) for r in rs])}")
