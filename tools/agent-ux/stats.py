"""Per exp/cond: mean ± sd of turns, output tokens, input+cache tokens, cost; and B-vs-A deltas with a crude separation flag."""
import json, sys, collections, statistics as st
def load(p): return [json.loads(l) for l in open(p)]
def ms(xs): return (st.mean(xs), st.stdev(xs) if len(xs) > 1 else 0.0)
for path, model in [(sys.argv[1], 'haiku'), (sys.argv[2], 'sonnet')]:
    g = collections.defaultdict(list)
    for r in load(path): g[(r['exp'], r['cond'])].append(r)
    print(f"\n### {model}\n")
    print("| Exp/cond | n | Turns | Output tokens | Input+cache tokens | Cost $ |")
    print("|---|---|---|---|---|---|")
    for (e, c), rs in sorted(g.items(), key=lambda k: (int(k[0][0][1:]), k[0][1])):
        cells = []
        for f, fmt in [('num_turns', '{:.1f} ± {:.1f}'), ('output_tokens', '{:,.0f} ± {:,.0f}'), ('input_plus_cache_tokens', '{:,.0f} ± {:,.0f}'), ('total_cost_usd', '{:.4f} ± {:.4f}')]:
            cells.append(fmt.format(*ms([r[f] for r in rs])))
        print(f"| {e}/{c} | {len(rs)} | " + " | ".join(cells) + " |")
