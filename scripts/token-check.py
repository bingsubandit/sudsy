"""Token and wake check of the agent host: counts the Claude Code session logs (`~/.claude/projects/**/*.jsonl`) and has one Haiku
call (scripts/haiku.py: SUDSY_API_KEY, no fallback) write a 5-line summary. Only counts go into the prompt: no file names, paths,
prompts or tool inputs.

  scripts/token-check.py START [END] [--dry-run]      START, END: ISO times in UTC; --dry-run prints the counts only (no key needed)

Cost units: weights opus 5, sonnet 3, haiku 1 (assumed relative prices); cost of a call = weight * (input + 1.25 * cache-write
+ 0.1 * cache-read + 5 * output) tokens / 1e6. A kind is what the call did (gate/finish/land, parity/corpus, git, cargo, edit, read,
other, text). A wake is a call more than 5 minutes (the prompt-cache lifetime) after the previous one of the same session: its context is
re-written to the cache, so wakes show the cost of waiting.
"""
import argparse
import collections
import datetime as dt
import glob
import json
import os
import re
import sys

import haiku

WEIGHT = {"opus": 5, "sonnet": 3, "haiku": 1, "fable": 10}
KINDS = (("gate/finish/land", r"finish\.sh|land\.sh|long\.sh --wait|gate\.sh"),
         ("parity/corpus/measure", r"fetch-|tarball|pip install|npm |parity\.sh|perf-bench|speed\.sh|evidence"),
         ("git", r"git (-C \S+ )?(rebase|add|commit|show|log|status|diff|fetch|checkout)"),
         ("cargo", r"cargo (build|test|run|check|xtask)|codegen"), ("edit", r"Edit:|Write:"),
         ("read/search", r"Read:|sed -n|grep|cat |rg "))
WAKE_SECONDS = 300
SYSTEM = ("You get counts of Claude Code token use on one machine (cost units, calls, wakes). Write exactly 5 lines, no heading: "
          "1 total and the split by model; 2 the kind of call that costs most; 3 the session that costs most; "
          "4 wakes (count, share of cost, what to change); 5 one concrete saving. Numbers only from the counts.")


def stamp(s):
    return dt.datetime.fromisoformat(s.replace("Z", "+00:00")).replace(tzinfo=dt.timezone.utc).timestamp()


def family(model):
    return next((k for k in WEIGHT if k in model), "?")


def kind_of(tools):
    s = " ".join(json.dumps(t["input"]) if t["name"] == "Bash" else t["name"] + ":" + str(t["input"].get("file_path", "")) for t in tools)
    for name, rx in KINDS:
        if re.search(rx, s):
            return name
    return "other" if tools else "text"


def read_calls(start, end, root):
    """message id -> [usage, tool uses, model, session, time]."""
    msgs = {}
    for f in glob.glob(os.path.join(root, "**/*.jsonl"), recursive=True):
        if os.path.getmtime(f) < start:
            continue
        with open(f, errors="ignore") as fh:
            lines = [ln for ln in fh if '"assistant"' in ln]
        for line in lines:
            try:
                d = json.loads(line)
            except ValueError:
                continue
            m = d.get("message") or {}
            if d.get("type") != "assistant" or not m.get("usage") or not start <= stamp(d["timestamp"]) < end:
                continue
            x = msgs.setdefault(m["id"], [m["usage"], [], m.get("model", ""), os.path.basename(f), stamp(d["timestamp"])])
            x[0] = m["usage"]
            x[1] += [c for c in m.get("content", []) if c.get("type") == "tool_use"]
    return msgs


def cost(u, model):
    return WEIGHT.get(family(model), 3) * (u.get("input_tokens", 0) + 1.25 * u.get("cache_creation_input_tokens", 0)
                                           + 0.1 * u.get("cache_read_input_tokens", 0) + 5 * u.get("output_tokens", 0)) / 1e6


def counts(msgs):
    total, model, kind, sess = 0.0, collections.Counter(), collections.Counter(), collections.Counter()
    wakes, wake_cost, last = 0, 0.0, {}
    for u, tools, mod, f, t in sorted(msgs.values(), key=lambda x: x[4]):
        c = cost(u, mod)
        total += c
        model[family(mod)] += c
        kind[kind_of(tools)] += c
        sess[f] += c
        if f in last and t - last[f] > WAKE_SECONDS:
            wakes += 1
            wake_cost += c
        last[f] = t
    top = [round(c, 1) for _, c in sess.most_common(5)]  # session names are left out of the prompt
    r = lambda m: {k: round(v, 1) for k, v in m.most_common()}
    return {"calls": len(msgs), "sessions": len(sess), "total_cost_units": round(total, 1), "by_model": r(model), "by_kind": r(kind),
            "top_session_costs": top, "wakes": wakes, "wake_cost_units": round(wake_cost, 1)}


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("start")
    ap.add_argument("end", nargs="?")
    ap.add_argument("--dry-run", action="store_true")
    ap.add_argument("--root", default=os.path.expanduser("~/.claude/projects"))
    a = ap.parse_args(argv)
    c = counts(read_calls(stamp(a.start), stamp(a.end) if a.end else 9e12, a.root))
    print(json.dumps(c))
    if a.dry_run:
        return 0
    try:
        res = haiku.call(json.dumps(c), system=SYSTEM, budget=0.1)
    except haiku.HaikuError as e:
        sys.exit(f"token-check: {e}")
    print(res.get("result", "").strip())
    print(f"token-check: {haiku.usage_line(res)}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
