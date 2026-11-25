"""Evidence run of a lint leg (docs/ai/development/no-reference-tool.md section 4): the primary linter over the corpus, every rule with its findings, the target and the plan to 85%, a seeded sample to read.

Usage: evidence_lint.py <language> <work-dir> [<n-sample> [<rule>|top:<n>:<k>]]
`top:<n>:<k>`: k findings of each of the n most frequent rules (drawn over distinct projects first), one source line each.

`<language>` names a module `scripts/evidence_<language>.py` (the same one scripts/evidence.py reads) with the facts of the tool:
  LINT      argv of the linter that takes a file and prints the findings (JSON) on stdout
  findings(stdout, path)   [{"line", "code", "name", "severity", "message", "context"}] of one run
  RULE      {code: (needs, verdict, note)}; verdict keep (the target), taste, bug, env, artifact; FAMILY {code: Sudsy family} (optional)
Runs are cached in <work-dir>/lint.json. Prints the verdicts with their findings, the rule table, the plan (kept rules in order of
findings with the cumulative share of the target, raw and weighted by project: every project counts as much as any other) and
a sample of findings with the source lines for a reader (seed SEED + 3).
"""
import importlib
import json
import random
import subprocess
import sys
from collections import Counter, defaultdict
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import evidence  # noqa: E402


def run(mod, work, corpus):
    cache = work / "lint.json"
    if cache.exists():
        return json.loads(cache.read_text())
    work.mkdir(parents=True, exist_ok=True)

    def one(entry):
        proj, name, f = entry
        try:
            r = subprocess.run([*mod.LINT, str(f)], cwd=work, capture_output=True, timeout=120, check=False)
        except subprocess.TimeoutExpired:
            return {"file": name, "project": proj, "failed": "timeout", "findings": []}
        out = r.stdout.decode("utf-8", "replace")
        try:
            return {"file": name, "project": proj, "findings": mod.findings(out, str(f))}
        except (ValueError, KeyError) as e:
            return {"file": name, "project": proj, "failed": f"{e}: {r.stderr.decode('utf-8', 'replace')[:200]}", "findings": []}

    with ThreadPoolExecutor(8) as ex:
        res = list(ex.map(one, corpus))
    cache.write_text(json.dumps(res))
    return res


def weighted(items, per_proj, nproj):
    """Share of a set of findings with every project counting the same."""
    return sum(c / per_proj[p] for p, c in Counter(r["project"] for r, _ in items).items()) / nproj


def main():
    lang, work = sys.argv[1], Path(sys.argv[2])
    n_sample = int(sys.argv[3]) if len(sys.argv) > 3 else 30
    only = sys.argv[4] if len(sys.argv) > 4 else None
    mod = importlib.import_module(f"evidence_{lang}")
    corpus = evidence.files(mod)
    deciding, held = evidence.split(corpus, mod.SEED)
    res = run(mod, work, corpus)
    path_of = {name: f for _, name, f in corpus}
    failed = [r for r in res if r.get("failed")]
    allf = [(r, f) for r in res for f in r["findings"]]
    print(f"{len(res)} files, {len({r['project'] for r in res})} projects, tool failed on {len(failed)} files; {len(allf)} findings in {sum(1 for r in res if r['findings'])} files")
    for r in failed[:10]:
        print(f"  failed: {r['file']}: {r['failed']}")
    by = defaultdict(list)
    for r, f in allf:
        by[f["code"]].append((r, f))
    if only and only.startswith("top:"):
        _, n, k = only.split(":")
        rng = random.Random(mod.SEED + 4)
        for code, items in sorted(by.items(), key=lambda kv: -len(kv[1]))[: int(n)]:
            print(f"\n## {code} {items[0][1]['name']} ({len(items)})")
            byp = defaultdict(list)
            for it in items:
                byp[it[0]["project"]].append(it)
            order = sorted(byp)
            rng.shuffle(order)
            for p in order[: int(k)]:
                r, f = rng.choice(byp[p])
                src = (evidence.read(path_of[r["file"]]) or "").split("\n")
                line = src[f["line"] - 1] if 0 < f["line"] <= len(src) else ""
                print(f"- {r['file'].split('/', 2)[2][-60:]}:{f['line']}: {f['message'][:110]!r} || {line.strip()[:130]!r}")
        return
    total = len(allf)
    per_proj = Counter(r["project"] for r, _ in allf)
    nproj = len(per_proj)
    rule = getattr(mod, "RULE", {})
    family = getattr(mod, "FAMILY", {})
    missing = sorted(c for c in by if c not in rule)
    if missing:
        print(f"rules with no verdict: {', '.join(missing)}")
    verdict_of = lambda c: rule.get(c, ("", "unjudged", ""))[1]  # noqa: E731
    print("\n| verdict | rules | findings | share | weighted by project | held-out findings |")
    print("|---|---|---|---|---|---|")
    for v in ("keep", "taste", "bug", "env", "artifact", "unjudged"):
        items = [x for c, xs in by.items() if verdict_of(c) == v for x in xs]
        if items:
            print(f"| {v} | {sum(1 for c in by if verdict_of(c) == v)} | {len(items)} | {100 * len(items) / total:.1f}% | {100 * weighted(items, per_proj, nproj):.1f}% | {sum(1 for r, _ in items if r['project'] in held)} |")
    print("\n| rule | name | severity | findings | files | projects | share | weighted by project | needs | verdict | note |")
    print("|---|---|---|---|---|---|---|---|---|---|---|")
    for code, items in sorted(by.items(), key=lambda kv: -len(kv[1])):
        k, v, note = rule.get(code, ("", "unjudged", ""))
        f0 = items[0][1]
        print(f"| {code} | {f0['name']} | {f0['severity']} | {len(items)} | {len({r['file'] for r, _ in items})} | {len({r['project'] for r, _ in items})} | {100 * len(items) / total:.1f}% | {100 * weighted(items, per_proj, nproj):.1f}% | {k} | {v} | {note.lstrip('*')}{'' if note.startswith('*') or not note else ''}{' (not read)' if note.startswith('*') else ''} |")
    for label, verdicts in (("target (keep and taste)", ("keep", "taste")), ("core (keep only)", ("keep",))):
        kept = [(c, xs) for c, xs in sorted(by.items(), key=lambda kv: -len(kv[1])) if verdict_of(c) in verdicts]
        target = [x for _, xs in kept for x in xs]
        held_target = sum(1 for r, _ in target if r["project"] in held)
        print(f"\n{label}: {len(target)} findings of {len(kept)} rules in {len({r['file'] for r, _ in target})} files; knowledge: " +
              ", ".join(f"{k} {sum(len(xs) for c, xs in kept if rule[c][0] == k)}" for k in ("line", "file", "flow")))
        if label.startswith("core"):
            continue
        print("\n| rank | rule | needs | verdict | findings | cumulative share of the target | weighted by project (cumulative) | held-out cumulative | Sudsy family |")
        print("|---|---|---|---|---|---|---|---|---|")
        cum, cum_held, cum_items = 0, 0, []
        for i, (code, xs) in enumerate(kept, 1):
            cum += len(xs)
            cum_held += sum(1 for r, _ in xs if r["project"] in held)
            cum_items += xs
            print(f"| {i} | {code} {xs[0][1]['name']} | {rule[code][0]} | {rule[code][1]} | {len(xs)} | {100 * cum / len(target):.1f}% | {100 * weighted(cum_items, per_proj, nproj) / weighted(target, per_proj, nproj):.1f}% | {100 * cum_held / max(held_target, 1):.1f}% | {family.get(code, 'pattern-rules (kind and regex rows)')} |")
    sample = random.Random(mod.SEED + 3).sample(allf, min(n_sample, len(allf)))
    print(f"\nsample of {len(sample)} findings:")
    for r, f in sample:
        src = (evidence.read(path_of[r["file"]]) or "").split("\n")
        ln = f["line"]
        ctx = "\n".join(f"      {i + 1}: {src[i][:160]}" for i in range(max(0, ln - 2), min(len(src), ln + 1)) if ln > 0)
        print(f"--- {r['file']}:{ln} {f['code']} {f['name']}: {f['message'][:200]}\n{ctx}")


if __name__ == "__main__":
    main()
