"""Sorts the differing files of a format parity run into the named gaps of tests/parity/<tool>/differences.yaml with a Haiku call
on the API credit (scripts/haiku.py; SUDSY_API_KEY, no fallback). Agents use it instead of reading the diffs themselves.

  PARITY_FILTER=<substring> scripts/parity.sh format <tool> > run.log      (prints `<file>: DIFFERENT` and a unified diff per file)
  scripts/classify.py <tool> run.log [--out res.json] [--max-files 20] [--max-tokens 40000] [--batch 5] [--dry-run]
  scripts/classify.py <tool> run.log --results res.json --spot 5            (no API call: N random results with their diffs)

Each file gets one gap name of the tool's `differences:` list or `unclassified`; the answer is JSON ({file: name}, then the counts as
`name:N` for the `files:` list). Cost cap: at most --max-files files and --max-tokens estimated input tokens (3 characters = 1 token,
the gap list counted in every batch) per run; files beyond a cap are not sent and are listed as skipped (raise the flags
for more). A diff is cut at 150 lines. Every call also has a dollar cap. Check the result with --spot before using it; above 10%
wrong, discard it.
"""
import argparse
import json
import random
import re
import sys
from collections import Counter
from pathlib import Path

import yaml

import haiku

NAME = re.compile(r"^([^\s+\-@].*?): (DIFFERENT|known difference|REVIEW)\b")
SCHEMA = {"type": "object", "required": ["classifications"], "properties": {"classifications": {"type": "array", "items": {
    "type": "object", "required": ["file", "gap"], "properties": {"file": {"type": "string"}, "gap": {"type": "string"}}}}}}
MAX_LINES = 150


def sections(text):
    """file -> its diff text, for the files the run calls DIFFERENT."""
    out, cur = {}, None
    for line in text.splitlines():
        m = NAME.match(line)
        if m:
            cur = m.group(1) if m.group(2) == "DIFFERENT" else None
            if cur:
                out[cur] = []
        elif cur:
            out[cur].append(line)
    return {n: "\n".join(d[:MAX_LINES]) + ("\n..." if len(d) > MAX_LINES else "") for n, d in out.items()}


def gaps(tool, root):
    doc = yaml.safe_load((root / "tests" / "parity" / tool / "differences.yaml").read_text()) or {}
    return {n: str(v) for n, v in (doc.get("differences") or {}).items()}


def system(names):
    lines = [f"- {n}: {t}" for n, t in names.items()]
    return ("You sort differences between a code formatter (Sudsy) and a reference formatter. Each file below has a unified diff: "
            "`-` lines are the reference tool's output, `+` lines are Sudsy's. For each file answer the one name from the list "
            "that explains its first (topmost) difference, or `unclassified` when no name fits. Never invent a name.\nNames:\n"
            + "\n".join(lines))


def estimate(text):
    return len(text) // 3 + 1


def plan(files, base_cost, max_files, max_tokens, batch):
    """(batches, skipped): files in order into batches of `batch` while both caps hold."""
    sent, tok = [], 0
    for name, diff in files.items():
        cost = estimate(name + diff)
        if len(sent) >= max_files or tok + cost > max_tokens:
            break
        sent.append((name, diff))
        tok += cost
    n = (len(sent) + batch - 1) // batch
    return [sent[i:i + batch] for i in range(0, len(sent), batch)], len(files) - len(sent), tok + n * base_cost


def classify(batches, names, budget):
    sysmsg, known, results, used = system(names), set(names), {}, 0
    for b in batches:
        res = haiku.call(json.dumps([{"file": n, "diff": d} for n, d in b]), system=sysmsg, schema=SCHEMA, budget=budget)
        used += haiku.tokens(res)
        print(f"classify: batch of {len(b)}: {haiku.usage_line(res)}", file=sys.stderr)
        for c in (res["data"] or {}).get("classifications", []):
            results[c["file"]] = c["gap"] if c["gap"] in known else "unclassified"
    for n, _ in (x for b in batches for x in b):
        results.setdefault(n, "unclassified")
    return results, used


def spot(results, diffs, n, seed=None):
    rng = random.Random(seed)
    for f in rng.sample(sorted(results), min(n, len(results))):
        print(f"=== {f}: {results[f]}\n{diffs.get(f, '(diff not in the log)')}\n")


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("tool")
    ap.add_argument("log", help="a format run's output, or - for stdin")
    ap.add_argument("--out")
    ap.add_argument("--results")
    ap.add_argument("--spot", type=int, default=0)
    ap.add_argument("--seed", type=int)
    ap.add_argument("--max-files", type=int, default=20)
    ap.add_argument("--max-tokens", type=int, default=40000)
    ap.add_argument("--batch", type=int, default=5)
    ap.add_argument("--budget", type=float, default=0.25, help="dollar cap of one call")
    ap.add_argument("--dry-run", action="store_true", help="print what would be sent and its estimate; no call")
    ap.add_argument("--root", default=str(Path(__file__).resolve().parent.parent))
    a = ap.parse_args(argv)
    diffs = sections(sys.stdin.read() if a.log == "-" else Path(a.log).read_text(errors="replace"))
    if a.spot:
        if not a.results:
            sys.exit("classify: --spot needs --results FILE (a result saved with --out)")
        spot(json.loads(Path(a.results).read_text()), diffs, a.spot, a.seed)
        return 0
    names = gaps(a.tool, Path(a.root))
    batches, skipped, est = plan(diffs, estimate(system(names)), a.max_files, a.max_tokens, a.batch)
    sent = sum(len(b) for b in batches)
    print(f"classify: {len(diffs)} differing files, sending {sent} in {len(batches)} batches (about {est} input tokens), "
          f"skipped {skipped} over the cap (--max-files {a.max_files}, --max-tokens {a.max_tokens})", file=sys.stderr)
    if a.dry_run:
        return 0
    try:
        haiku.require_key()
        results, used = classify(batches, names, a.budget)
    except haiku.HaikuError as e:
        sys.exit(f"classify: {e}")
    if a.out:
        Path(a.out).write_text(json.dumps(results, indent=1) + "\n")
    print(json.dumps(results, indent=1))
    print(", ".join(f"{n}:{c}" for n, c in Counter(results.values()).most_common()))
    print(f"classify: {used} tokens in total", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
