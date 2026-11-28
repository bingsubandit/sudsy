"""bench.py [--smoke | --full] [--conditions C,...] [options]: one batch of the agent bench.

Plans every task under every condition in each round, in a seeded random order per round (paired and
interleaved), runs headless Claude Code on a fresh run directory per job with the condition's allowlist
and hooks (normal permission checks), verifies the result, and appends one JSON line per run to
data/<date>-<sudsy sha>[-smoke].jsonl. Transcripts stay in the batch's work directory."""
import argparse
import collections
import concurrent.futures as cf
import datetime as dt
import json
import os
import pathlib
import random
import subprocess
import sys
import threading
import time

import conditions
import fetch
from common import HERE, REPO_ROOT, load_task, lock, repos, run_env, task_ids, work_root
from setup_run import setup
from verify import verify

DATA = HERE / "data"
WRITE = threading.Lock()


def claude_version():
    r = subprocess.run(["claude", "--version"], capture_output=True, text=True)
    return r.stdout.strip().split()[0] if r.returncode == 0 else None


def sudsy_info(path):
    v = subprocess.run([path, "--version"], capture_output=True, text=True).stdout.strip()
    sha = subprocess.run(["git", "rev-parse", "HEAD"], cwd=REPO_ROOT, capture_output=True, text=True).stdout.strip()
    dirty = bool(subprocess.run(["git", "status", "--porcelain", "--", "crates"], cwd=REPO_ROOT, capture_output=True, text=True).stdout.strip())
    return {"sudsy_path": str(path), "sudsy_version": v, "sudsy_commit": sha, "sudsy_dirty": dirty}


def plan(tasks, conds, reps, seed):
    """Rounds of every (task, condition) pair, each round in its own seeded order."""
    jobs = []
    for rep in range(1, reps + 1):
        pairs = [(t, c) for t in tasks for c in conds]
        random.Random(f"{seed}-{rep}").shuffle(pairs)
        jobs += [(t, c, rep) for t, c in pairs]
    return jobs


def parse_stream(path):
    """The final result event and per-tool call counts of a stream-json transcript."""
    result, tools = {}, collections.Counter()
    for line in path.read_text(errors="replace").splitlines():
        try:
            ev = json.loads(line)
        except ValueError:
            continue
        if ev.get("type") == "result":
            result = ev
        elif ev.get("type") == "assistant":
            for block in (ev.get("message") or {}).get("content") or []:
                if block.get("type") == "tool_use":
                    name = block.get("name")
                    if name == "Bash":
                        cmd = (block.get("input") or {}).get("command", "").split()
                        name = "Bash:" + " ".join(cmd[:2])
                    tools[name] += 1
    return result, dict(tools)


def metrics(result):
    u = result.get("usage") or {}
    denials = result.get("permission_denials") or []
    return {
        "num_turns": result.get("num_turns"),
        "output_tokens": u.get("output_tokens"),
        "input_tokens": u.get("input_tokens"),
        "cache_creation_input_tokens": u.get("cache_creation_input_tokens"),
        "cache_read_input_tokens": u.get("cache_read_input_tokens"),
        "total_cost_usd": result.get("total_cost_usd"),
        "duration_ms": result.get("duration_ms"),
        "is_error": result.get("is_error", True),
        "result_subtype": result.get("subtype"),
        "model_ids": sorted((result.get("modelUsage") or {}).keys()),
        "permission_denials": len(denials),
        "denied": [(d.get("tool_name"), str((d.get("tool_input") or {}).get("command", ""))[:120]) for d in denials],
    }


def run_one(job, ctx):
    task_id, cond, rep, order = job
    task = load_task(ctx["version"], task_id)
    run, meta = setup(ctx["version"], task_id, cond, rep, ctx["root"], ctx["sudsy"])
    bin_dir = meta / "bin"
    bin_dir.mkdir()
    if conditions.family_name(cond) == "sudsy":
        (bin_dir / "sudsy").symlink_to(ctx["sudsy"])
    env = run_env({**repos()["repo"][task["repo"]], "name": task["repo"]}, extra_path=[bin_dir])
    args = ["claude", "-p", (meta / "TASK.md").read_text(), "--model", ctx["model"],
            "--output-format", "stream-json", "--verbose",
            "--allowedTools", conditions.allowed_tools(task["repo"], cond),
            "--permission-mode", "default", "--setting-sources", "project", "--strict-mcp-config",
            "--no-session-persistence", "--max-budget-usd", str(ctx["max_budget"])]
    if conditions.settings(cond, meta):
        args += ["--settings", str(meta / "settings.json")]
    transcript = meta / "transcript.jsonl"
    t0 = time.time()
    with open(transcript, "w") as out, open(meta / "stderr.txt", "w") as err:
        try:
            code = subprocess.run(args, cwd=run, env=env, stdout=out, stderr=err, stdin=subprocess.DEVNULL,
                                  timeout=ctx["timeout"]).returncode
        except subprocess.TimeoutExpired:
            code = "timeout"
    wall = time.time() - t0
    result, tools = parse_stream(transcript)
    rec = {**ctx["batch_info"], "task": task_id, "kind": task["kind"], "repo": task["repo"],
           "cond": conditions.RESULT_NAME.get(cond, cond), "rep": rep, "order": order, "exit": code,
           "wall_s": round(wall, 1), **metrics(result), "tool_calls": tools,
           "hook_calls": sum(1 for _ in (meta / "calls.log").read_text().splitlines()),
           "transcript": str(transcript)}
    try:
        rec.update(verify(run, meta, ctx["sudsy"]))
    except Exception as e:  # noqa: BLE001 - a verifier crash is recorded as a failed run
        rec.update({"success": False, "verify_error": str(e)})
    if code == "timeout":
        rec["success"] = False
    with WRITE:
        with open(ctx["out"], "a", newline="\n") as f:
            f.write(json.dumps(rec) + "\n")
        ctx["spent"] += rec.get("total_cost_usd") or 0
    return rec


def main():
    lk = lock()
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    mode = ap.add_mutually_exclusive_group()
    mode.add_argument("--smoke", action="store_true", help="the smoke set on the smoke model (default)")
    mode.add_argument("--full", action="store_true", help="every task, the full reps, the published model")
    ap.add_argument("--conditions", default=",".join(conditions.CONDITIONS))
    ap.add_argument("--tasks", help="comma-separated task ids (default: the mode's set)")
    ap.add_argument("--reps", type=int)
    ap.add_argument("--model")
    ap.add_argument("--parallel", type=int, default=4)
    ap.add_argument("--seed", default="1")
    ap.add_argument("--sudsy", help="the sudsy binary (default: the slot's release build)")
    ap.add_argument("--max-cost", type=float, help="start no new run once the batch spent this many dollars")
    ap.add_argument("--label", default="", help="appended to the data file name")
    ap.add_argument("--dry-run", action="store_true", help="print the plan and the instruction blocks only")
    a = ap.parse_args()
    m = "full" if a.full else "smoke"
    version = lk["task_set"]
    conds = a.conditions.split(",")
    for c in conds:
        if c not in conditions.CONDITIONS:
            sys.exit(f"unknown condition {c}; known: {', '.join(conditions.CONDITIONS)}")
    all_tasks = task_ids(version)
    if a.tasks:
        tasks = a.tasks.split(",")
    elif m == "smoke":
        tasks = sorted(random.Random("smoke").sample(all_tasks, min(lk["smoke"]["tasks"], len(all_tasks))))
    else:
        tasks = all_tasks
    reps = a.reps or lk[m]["reps"]
    sudsy = pathlib.Path(a.sudsy or (work_root().parent / "release" / "sudsy")).resolve()
    jobs = [(t, c, r, i) for i, (t, c, r) in enumerate(plan(tasks, conds, reps, a.seed))]
    if a.dry_run:
        for j in jobs:
            print(*j)
        for c in conds:
            print(f"--- {c}\n" + conditions.instructions(load_task(version, tasks[0])["repo"], c))
        return
    if not sudsy.exists():
        sys.exit(f"no sudsy binary at {sudsy}: CARGO_TARGET_DIR=<slot> cargo build --release -p cli, or --sudsy")
    cv = claude_version()
    if cv != lk["claude_code"]:
        print(f"warning: Claude Code {cv}, the lockfile pins {lk['claude_code']}; recorded per run", file=sys.stderr)
    fetch.main(sorted({load_task(version, t)["repo"] for t in tasks}))
    info = sudsy_info(str(sudsy))
    date = dt.date.today().isoformat()
    name = f"{date}-{info['sudsy_commit'][:10]}{'-' + m if m == 'smoke' else ''}{'-' + a.label if a.label else ''}"
    root = work_root() / "batches" / name
    root.mkdir(parents=True, exist_ok=True)
    DATA.mkdir(exist_ok=True)
    ctx = {"version": version, "root": root, "sudsy": str(sudsy), "model": a.model or lk["model"][m],
           "timeout": lk["timeout_s"], "max_budget": lk["max_budget_usd"][m], "out": DATA / f"{name}.jsonl", "spent": 0.0,
           "batch_info": {"batch": name, "mode": m, "task_set": version, "claude_code": cv, "model_requested": a.model or lk["model"][m],
                          "seed": a.seed, **info}}
    print(f"batch {name}: {len(tasks)} tasks x {len(conds)} conditions x {reps} reps = {len(jobs)} runs -> {ctx['out']}")
    with cf.ThreadPoolExecutor(max_workers=a.parallel) as pool:
        futs = []
        for j in jobs:
            while len([f for f in futs if not f.done()]) >= a.parallel:
                time.sleep(1)
            if a.max_cost is not None and ctx["spent"] >= a.max_cost:
                print(f"stopping: ${ctx['spent']:.2f} spent (--max-cost {a.max_cost})")
                break
            futs.append(pool.submit(run_one, j, ctx))
        for f in cf.as_completed(futs):
            try:
                r = f.result()
                print(f"{r['task']} {r['cond']} {r['rep']}: success={r.get('success')} turns={r.get('num_turns')} "
                      f"cost={r.get('total_cost_usd')} denials={r.get('permission_denials')}", flush=True)
            except Exception as e:  # noqa: BLE001
                print(f"run failed: {e}", flush=True)
    print(f"done: ${ctx['spent']:.2f}; python3 {HERE / 'stats.py'} {ctx['out']}")


if __name__ == "__main__":
    os.chdir(HERE)
    main()
