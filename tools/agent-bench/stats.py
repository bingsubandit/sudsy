"""stats.py BATCH.jsonl [--base native] | --compare A.jsonl B.jsonl: the numbers of a batch.

Per metric and condition against the base condition: the ratio of the geometric means over tasks of the
per-task means (tasks that have runs in both), with a 95% bootstrap interval that resamples tasks and,
within each task, reps. A ratio is publishable when its interval lies within +-5% of the ratio
(BAR). Success rates with Wilson 95% intervals; the verifier's mistake counters per condition.
--compare: the repeatability check of two batches of the same versions: every ratio of A must lie in
B's interval and the other way round."""

import argparse
import collections
import json
import math
import random

METRICS = [
    "num_turns",
    "output_tokens",
    "input_tokens",
    "cache_read_input_tokens",
    "cache_creation_input_tokens",
    "total_cost_usd",
    "duration_ms",
]
BAR = 0.05
BOOT = 2000


def load(path):
    return [json.loads(l) for l in open(path) if l.strip()]


def by_task(rows, cond, metric):
    g = collections.defaultdict(list)
    for r in rows:
        v = r.get(metric)
        if r["cond"] == cond and isinstance(v, (int, float)) and v > 0:
            g[r["task"]].append(v)
    return g


def gm_ratio(a, b, tasks):
    """exp(mean log mean(b_t) - mean log mean(a_t)): condition b against a."""
    return math.exp(
        sum(
            math.log(sum(b[t]) / len(b[t])) - math.log(sum(a[t]) / len(a[t]))
            for t in tasks
        )
        / len(tasks)
    )


def ratio_ci(rows, base, cond, metric, seed=0):
    a, b = by_task(rows, base, metric), by_task(rows, cond, metric)
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
        "publishable": lo >= point * (1 - BAR) and hi <= point * (1 + BAR),
    }


def wilson(k, n, z=1.96):
    if n == 0:
        return (0.0, 0.0, 0.0)
    p = k / n
    d = 1 + z * z / n
    c = (p + z * z / (2 * n)) / d
    h = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / d
    return (p, max(0.0, c - h), min(1.0, c + h))


def conds_of(rows):
    return sorted({r["cond"] for r in rows})


def report(rows, base):
    out = []
    conds = conds_of(rows)
    b0 = rows[0]
    out.append(
        f"batch {b0['batch']}: {len(rows)} runs, {len({r['task'] for r in rows})} tasks, Claude Code {b0['claude_code']}, "
        f"models {sorted({m for r in rows for m in r.get('model_ids') or []})}, sudsy {b0['sudsy_commit'][:10]}, "
        f"cost ${sum(r.get('total_cost_usd') or 0 for r in rows):.2f}"
    )
    out.append(
        "\n| Condition | Runs | Success (95% CI) | Unformatted | New findings | Out of scope | Runs with denials |"
    )
    out.append("|---|---|---|---|---|---|---|")
    for c in conds:
        rs = [r for r in rows if r["cond"] == c]
        p, lo, hi = wilson(sum(bool(r.get("success")) for r in rs), len(rs))
        out.append(
            f"| {c} | {len(rs)} | {p:.0%} ({lo:.0%}-{hi:.0%}) | {sum(not r.get('formatted', True) for r in rs)} | "
            f"{sum(not r.get('lint_clean', True) for r in rs)} | {sum(bool(r.get('out_of_scope')) for r in rs)} | "
            f"{sum((r.get('permission_denials') or 0) > 0 for r in rs)} |"
        )
    others = [c for c in conds if c != base]
    if base in conds and others:
        out.append(
            f"\nRatio of geometric means against `{base}` (95% bootstrap interval; * = within the +-{BAR:.0%} bar)\n"
        )
        out.append("| Metric | " + " | ".join(others) + " |")
        out.append("|---|" + "---|" * len(others))
        for m in METRICS:
            cells = []
            for c in others:
                ci = ratio_ci(rows, base, c, m)
                cells.append(
                    "-"
                    if ci is None
                    else f"{ci['ratio'] - 1:+.0%} ({ci['lo'] - 1:+.0%} to {ci['hi'] - 1:+.0%}){' *' if ci['publishable'] else ''}"
                )
            out.append(f"| {m} | " + " | ".join(cells) + " |")
    return "\n".join(out)


def compare(rows_a, rows_b, base):
    """Repeatability: every ratio of each batch inside the other's interval."""
    out, agree = [], True
    for c in sorted(set(conds_of(rows_a)) & set(conds_of(rows_b)) - {base}):
        for m in METRICS:
            a, b = ratio_ci(rows_a, base, c, m), ratio_ci(rows_b, base, c, m)
            if a is None or b is None:
                continue
            ok = b["lo"] <= a["ratio"] <= b["hi"] and a["lo"] <= b["ratio"] <= a["hi"]
            agree &= ok
            out.append(
                f"{c:>26} {m:<28} A {a['ratio'] - 1:+.1%} [{a['lo'] - 1:+.1%}, {a['hi'] - 1:+.1%}]  "
                f"B {b['ratio'] - 1:+.1%} [{b['lo'] - 1:+.1%}, {b['hi'] - 1:+.1%}]  {'agree' if ok else 'DIFFER'}"
            )
    out.append("repeatable: " + ("yes" if agree else "no"))
    return "\n".join(out), agree


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("batch", nargs="?")
    ap.add_argument("--base", default="native")
    ap.add_argument("--compare", nargs=2, metavar=("A", "B"))
    a = ap.parse_args()
    if a.compare:
        text, ok = compare(load(a.compare[0]), load(a.compare[1]), a.base)
        print(text)
        raise SystemExit(0 if ok else 1)
    print(report(load(a.batch), a.base))


if __name__ == "__main__":
    main()
