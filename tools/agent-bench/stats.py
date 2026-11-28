"""stats.py BATCH.jsonl [BATCH.jsonl...] | --compare A.jsonl B.jsonl: the numbers of a batch, per pair.

Per pair of pairs.toml (its `on` arm against its `off` arm, on the tasks that list the pair) and metric:
the ratio of the geometric means over tasks of the per-task means, with a 95% bootstrap interval that
resamples tasks and, within each task, reps. A ratio is publishable when its interval lies within +-5%
of it (BAR) and it has at least MIN_TASKS tasks. Success rates with Wilson 95% intervals, the verifier's mistake counters, and the size
estimate: from the batch's spread, the reps per task (at the pair's task count) or the tasks (at 5 reps)
the bar needs. --compare: the repeatability check of two batches of the same versions: every ratio of A
must lie in B's interval and the other way round."""

import argparse
import collections
import json
import math
import random
import statistics

from arms import pairs
from common import load_task, task_ids

METRICS = [
    "num_turns",
    "output_tokens",
    "input_tokens",
    "cache_read_input_tokens",
    "cache_creation_input_tokens",
    "total_cost_usd",
    "duration_ms",
]
PLAN_METRICS = ["num_turns", "total_cost_usd"]
BAR = 0.05
BOOT = 2000
MIN_TASKS = 5  # fewer tasks give an interval too narrow to trust


def load(paths):
    return [json.loads(l) for p in paths for l in open(p) if l.strip()]


def pair_rows(rows, name):
    """The runs of a pair: its two arms on the tasks of the pair that ran under both."""
    p = pairs()[name]
    rs = [
        r
        for r in rows
        if name in (r.get("pairs") or []) and r["cond"] in (p["on"], p["off"])
    ]
    both = {
        t
        for t in {r["task"] for r in rs}
        if {r["cond"] for r in rs if r["task"] == t} == {p["on"], p["off"]}
    }
    return [r for r in rs if r["task"] in both]


def by_task(rows, arm, metric):
    g = collections.defaultdict(list)
    for r in rows:
        v = r.get(metric)
        if r["cond"] == arm and isinstance(v, (int, float)) and v > 0:
            g[r["task"]].append(v)
    return g


def gm_ratio(a, b, tasks):
    """exp(mean log mean(b_t) - mean log mean(a_t)): arm b against arm a."""
    return math.exp(
        sum(
            math.log(sum(b[t]) / len(b[t])) - math.log(sum(a[t]) / len(a[t]))
            for t in tasks
        )
        / len(tasks)
    )


def ratio_ci(rows, off, on, metric, seed=0):
    a, b = by_task(rows, off, metric), by_task(rows, on, metric)
    tasks = sorted(set(a) & set(b))
    if not tasks:
        return None
    point = gm_ratio(a, b, tasks)
    rnd = random.Random(seed)
    boots = []
    for _ in range(BOOT):
        ts = [rnd.choice(tasks) for _ in tasks]
        ra = {i: [rnd.choice(a[t]) for _ in a[t]] for i, t in enumerate(ts)}
        rb = {i: [rnd.choice(b[t]) for _ in b[t]] for i, t in enumerate(ts)}
        boots.append(gm_ratio(ra, rb, range(len(ts))))
    boots.sort()
    lo, hi = boots[int(0.025 * BOOT)], boots[int(0.975 * BOOT) - 1]
    return {
        "ratio": point,
        "lo": lo,
        "hi": hi,
        "tasks": len(tasks),
        "publishable": len(tasks) >= MIN_TASKS
        and lo >= point * (1 - BAR)
        and hi <= point * (1 + BAR),
    }


def size_estimate(rows, off, on, metric, t_full):
    """(reps per task the bar needs with all `t_full` tasks of the pair, or None when no rep count
    reaches it; tasks it needs at 5 reps): half-width = 1.96 * sqrt((s_b^2 + 2 s_w^2 / R) / T) on the log scale, with
    s_w^2 the within-(task, arm) variance of log metric and s_b^2 the between-task variance of the
    per-task log ratio less its rep noise."""
    a, b = by_task(rows, off, metric), by_task(rows, on, metric)
    tasks = [t for t in set(a) & set(b) if len(a[t]) > 1 and len(b[t]) > 1]
    if len(tasks) < 2:
        return None
    sw2 = statistics.mean(
        statistics.variance([math.log(v) for v in g[t]]) for g in (a, b) for t in tasks
    )
    reps = statistics.mean(len(g[t]) for g in (a, b) for t in tasks)
    d = [
        statistics.mean(math.log(v) for v in b[t])
        - statistics.mean(math.log(v) for v in a[t])
        for t in tasks
    ]
    sb2 = max(0.0, statistics.variance(d) - 2 * sw2 / reps)
    hw = math.log(1 + BAR) / 1.96
    room = hw * hw * t_full - sb2
    r_need = math.ceil(2 * sw2 / room) if room > 0 else None
    t_at5 = math.ceil((sb2 + 2 * sw2 / 5) / (hw * hw))
    return {
        "reps": r_need,
        "tasks_at_5_reps": t_at5,
        "tasks": len(tasks),
        "t_full": t_full,
    }


def wilson(k, n, z=1.96):
    if n == 0:
        return (0.0, 0.0, 0.0)
    p = k / n
    d = 1 + z * z / n
    c = (p + z * z / (2 * n)) / d
    h = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / d
    return (p, max(0.0, c - h), min(1.0, c + h))


def pct(ci):
    if ci is None:
        return "-"
    return f"{ci['ratio'] - 1:+.0%} ({ci['lo'] - 1:+.0%} to {ci['hi'] - 1:+.0%}){' *' if ci['publishable'] else ''}"


def report(rows, task_counts):
    out = []
    b0 = rows[0]
    out.append(
        f"batch {b0['batch']}: {len(rows)} runs, {len({r['task'] for r in rows})} tasks, Claude Code {b0['claude_code']}, "
        f"models {sorted({m for r in rows for m in r.get('model_ids') or []})}, sudsy {b0['sudsy_commit'][:10]}, "
        f"cost ${sum(r.get('total_cost_usd') or 0 for r in rows):.2f}"
    )
    out.append(
        "\n| Arm | Runs | Success (95% CI) | Unformatted | New findings | Out of scope | Policy violations | Runs with denials |"
    )
    out.append("|---|---|---|---|---|---|---|---|")
    for c in sorted({r["cond"] for r in rows}):
        rs = [r for r in rows if r["cond"] == c]
        p, lo, hi = wilson(sum(bool(r.get("success")) for r in rs), len(rs))
        out.append(
            f"| {c} | {len(rs)} | {p:.0%} ({lo:.0%}-{hi:.0%}) | {sum(not r.get('formatted', True) for r in rs)} | "
            f"{sum(not r.get('lint_clean', True) for r in rs)} | {sum(bool(r.get('out_of_scope')) for r in rs)} | "
            f"{sum(len(r.get('policy_violations') or []) for r in rs)} | {sum((r.get('permission_denials') or 0) > 0 for r in rs)} |"
        )
    out.append(
        f"\nPer pair: `on` against `off`, ratio of geometric means (95% bootstrap interval; * = within the +-{BAR:.0%} bar)\n"
    )
    out.append("| Pair | Tasks | Success on / off | " + " | ".join(METRICS) + " |")
    out.append("|---|---|---|" + "---|" * len(METRICS))
    sizes = []
    for name, p in pairs().items():
        rs = pair_rows(rows, name)
        if not rs:
            continue
        succ = []
        for arm in (p["on"], p["off"]):
            ar = [r for r in rs if r["cond"] == arm]
            succ.append(f"{sum(bool(r.get('success')) for r in ar)}/{len(ar)}")
        cis = [ratio_ci(rs, p["off"], p["on"], m) for m in METRICS]
        n = max((c["tasks"] for c in cis if c), default=0)
        if n == 0:
            continue
        out.append(
            f"| {name} | {n} | {' / '.join(succ)} | "
            + " | ".join(pct(c) for c in cis)
            + " |"
        )
        for m in PLAN_METRICS:
            e = size_estimate(rs, p["off"], p["on"], m, task_counts.get(name, 0))
            if e:
                sizes.append(
                    f"| {name} | {m} | {e['tasks']} | {e['t_full']} | {e['reps'] if e['reps'] else 'not reachable'} | {e['tasks_at_5_reps']} |"
                )
    if sizes:
        out.append(f"\nSize for the +-{BAR:.0%} bar from this batch's spread\n")
        out.append(
            "| Pair | Metric | Tasks measured | Tasks in the set | Reps needed with all of them | Tasks needed at 5 reps |"
        )
        out.append("|---|---|---|---|---|---|")
        out += sizes
    return "\n".join(out)


def compare(rows_a, rows_b):
    """Repeatability: every ratio of each batch inside the other's interval."""
    out, agree = [], True
    for name, p in pairs().items():
        ra, rb = pair_rows(rows_a, name), pair_rows(rows_b, name)
        for m in METRICS:
            a, b = ratio_ci(ra, p["off"], p["on"], m), ratio_ci(
                rb, p["off"], p["on"], m
            )
            if a is None or b is None:
                continue
            ok = b["lo"] <= a["ratio"] <= b["hi"] and a["lo"] <= b["ratio"] <= a["hi"]
            agree &= ok
            out.append(
                f"{name:>8} {m:<28} A {a['ratio'] - 1:+.1%} [{a['lo'] - 1:+.1%}, {a['hi'] - 1:+.1%}]  "
                f"B {b['ratio'] - 1:+.1%} [{b['lo'] - 1:+.1%}, {b['hi'] - 1:+.1%}]  {'agree' if ok else 'DIFFER'}"
            )
    out.append("repeatable: " + ("yes" if agree else "no"))
    return "\n".join(out), agree


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("batch", nargs="*")
    ap.add_argument("--compare", nargs=2, metavar=("A", "B"))
    a = ap.parse_args()
    if a.compare:
        text, ok = compare(load([a.compare[0]]), load([a.compare[1]]))
        print(text)
        raise SystemExit(0 if ok else 1)
    rows = load(a.batch)
    version = rows[0]["task_set"]
    counts = {}
    for t in task_ids(version):
        for p in load_task(version, t).get("pairs", []):
            counts[p] = counts.get(p, 0) + 1
    print(report(rows, counts))


if __name__ == "__main__":
    main()
