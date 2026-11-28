"""Change of each condition against condition A of the same experiment, per metric.

A change is marked `clear` when |mean_X - mean_A| > sd_X + sd_A (a crude separation rule for n=3),
`~` otherwise. Usage: python3 deltas.py data/haiku.jsonl [more.jsonl ...]
"""
import collections
import json
import statistics as st
import sys

METRICS = [("num_turns", "turns"), ("output_tokens", "out"), ("input_plus_cache_tokens", "in"), ("total_cost_usd", "cost")]


def ms(xs):
    return st.mean(xs), (st.stdev(xs) if len(xs) > 1 else 0.0)


for path in sys.argv[1:]:
    g = collections.defaultdict(list)
    for line in open(path):
        r = json.loads(line)
        g[(r["exp"], r["cond"])].append(r)
    print(f"\n{path}\n| Exp | vs A | " + " | ".join(m for _, m in METRICS) + " |")
    print("|---|---|" + "---|" * len(METRICS))
    for (e, c), rs in sorted(g.items(), key=lambda k: (int(k[0][0][1:]), k[0][1])):
        if c == "A" or (e, "A") not in g:
            continue
        base = g[(e, "A")]
        cells = []
        for f, _ in METRICS:
            ma, sa = ms([r[f] for r in base])
            mx, sx = ms([r[f] for r in rs])
            pct = (mx - ma) / ma * 100
            cells.append(f"{pct:+.0f}%" + (" clear" if abs(mx - ma) > sa + sx else ""))
        print(f"| {e} | {c} | " + " | ".join(cells) + " |")
