#!/usr/bin/env python3
"""Speed of `sudsy format --check` and `sudsy lint` against the tools they replace (docs/ai/design/speed-comparison.md).

Usage: speed.py <sudsy> <target dir> [--out FILE] [tool...]. Every input is the `speed` key of a tool entry of
scripts/parity_tools.json (docs/ai/development/parity.md#speed): its directory-mode commands, its corpus and its
copy count. Sudsy and the tool alternate, a new process each run; the result is the best wall time of `runs` runs
and the peak resident memory of that run, written to bench/speed.json (the README table is generated from it)."""
import argparse
import json
import os
import platform
import shutil
import subprocess
import sys
import time
from collections import Counter
from pathlib import Path

import parity_format as pf

ROOT = pf.ROOT
RUNS = 5


def entries():
    """name -> (config, speed) for every entry with a `speed` key, in file order. A tool with a parity entry carries
    the key there; one without (oxlint, dprint) is an entry of the `speed` section, which no parity runner reads."""
    found = {}
    for section in ("format", "lint", "speed"):
        for name, config in pf.tools().get(section, {}).items():
            if "speed" in config:
                found.setdefault(name, (config, config["speed"]))
    return found


def prepare(name, config, speed, parity):
    """Builds what the entry needs, fetches its corpus and installs the tool; returns (version, prefix, exe)."""
    if "worker" in config.get("needs", []):
        pf.run(["dotnet", "build", "-c", "Release", "--nologo", "-v:q", "workers/csharp/Sudsy.Worker"], ROOT)
        os.environ.setdefault("SUDSY_CSHARP_WORKER", str(ROOT / "workers/csharp/Sudsy.Worker/bin/Release/net10.0/Sudsy.Worker.dll"))
    for script in config.get("fetch", []):
        pf.run(["bash", script], ROOT)
    catalog = config.get("catalog")
    version = config.get("version") or pf.catalog_version(catalog)
    prefix = parity / f"{catalog or name}-{version}"
    spec = config.get("install")
    spec = pf.tools()["format"][spec]["install"] if isinstance(spec, str) else spec
    if spec:
        pf.install(spec, catalog or name, version, prefix)
    for file, text in speed.get("files", {}).items():  # config files the tool's command reads
        (prefix / file).write_text(text.replace("{prefix}", str(prefix)), encoding="utf-8", newline="\n")
    return version, prefix, pf.executable(prefix, spec["exe"]) if spec and "exe" in spec else ""


def corpus(name, config, speed, work):
    """Copies the speed corpus into `work` (`copies` times) and returns (files, bytes, text). `corpus` names an entry of
    `corpora` in parity_tools.json (fixture sources shared by the tools of a language), or is `{"group": <ratchet group
    of the entry's fixtures>}` (`fixtures`: the ungrouped ones)."""
    spec = speed["corpus"]
    if isinstance(spec, str):
        spec = pf.tools()["corpora"][spec]
    else:
        spec = {**spec, "sources": [s for s in config["fixtures"] if (s.get("ratchet") or "fixtures") == spec["group"]]}
    for script in spec.get("fetch", []):
        pf.run(["bash", script], ROOT)
    found = pf.fixtures({"fixtures": spec["sources"]}, ROOT / "tests/parity" / name, Counter())
    if not found:
        raise SystemExit(f"{name}: the speed corpus is empty")
    copies = spec.get("copies", 1)
    shutil.rmtree(work, ignore_errors=True)
    size = 0
    for i in range(copies):
        for rel, path, _ in found:
            dest = work / (f"c{i + 1}" if copies > 1 else "") / rel
            dest.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(path, dest)
            size += path.stat().st_size
    return len(found) * copies, size, spec.get("text") or f"{len(found)} files"


def measure(command, cwd, env):
    """One run in a new process: (wall seconds, peak resident MB of the process and the children it waited for)."""
    start = time.perf_counter()
    p = subprocess.Popen(command, cwd=cwd, env=env, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    _, status, usage = os.wait4(p.pid, 0)
    seconds = time.perf_counter() - start
    p.returncode = os.waitstatus_to_exitcode(status)
    if p.returncode in (126, 127):
        raise SystemExit(f"{' '.join(map(str, command))}: cannot run")
    return seconds, usage.ru_maxrss / 1024  # ru_maxrss is KiB on Linux


def best_of(sudsy_cmd, tool_cmd, cwd, sudsy_env, tool_env):
    """Alternates the two commands; the best run of each by wall time, as {"s", "mb"}."""
    best = {}
    for _ in range(RUNS):
        for key, command, env in (("sudsy", sudsy_cmd, sudsy_env), ("other", tool_cmd, tool_env)):
            seconds, mb = measure(command, cwd, env)
            if key not in best or seconds < best[key]["s"]:
                best[key] = {"s": round(seconds, 3), "mb": round(mb)}
    return best


def runner():
    cpu = next((l.split(":", 1)[1].strip() for l in open("/proc/cpuinfo") if l.startswith(("model name", "Model name", "CPU part"))), "")
    return os.environ.get("SPEED_RUNNER") or f"{platform.system()} {platform.machine()}, {os.cpu_count()} cores{', ' + cpu if cpu else ''}"


def main():
    sys.stdout.reconfigure(encoding="utf-8")
    ap = argparse.ArgumentParser()
    ap.add_argument("sudsy")
    ap.add_argument("target")
    ap.add_argument("--out", default=str(ROOT / "bench/speed.json"))
    ap.add_argument("tools", nargs="*")
    args = ap.parse_args()
    known = entries()
    unknown = [t for t in args.tools if t not in known]
    if unknown:
        raise SystemExit(f"no `speed` key: {', '.join(unknown)} (measured: {', '.join(known)})")
    chosen = args.tools or list(known)
    parity = Path(args.target).resolve() / "parity"
    lint_presets = {n: c.get("preset") for n, c in pf.tools()["lint"].items()}
    rows = []
    for name in chosen:
        config, speed = known[name]
        version, prefix, exe = prepare(name, config, speed, parity)
        tasks = {}  # task -> (the sudsy.yaml of the run or None, the Sudsy command)
        for task in ("format", "lint"):
            if task == "format":
                folder = ROOT / "tests/parity" / speed.get("sudsy", name) / "sudsy.yaml"
                tasks[task] = (folder.read_text(encoding="utf-8") if folder.exists() else None, ["{sudsy}", "format", "--check", "."])
            else:
                preset = lint_presets.get(name)
                tasks[task] = (json.dumps({"extends": [f"preset:{preset}"]}) if preset else None, ["{sudsy}", "lint", "."])
        work = parity / f"{name}-speed"
        corpus_info = None
        values = {"prefix": prefix, "exe": exe, "python": sys.executable, "root": ROOT, "sudsy": args.sudsy}
        fill = lambda cmd: [str(a).format(**values) for a in cmd]  # noqa: E731
        tool_env = {**os.environ, **{k: v.format(prefix=prefix) for k, v in config.get("env", {}).items()}}
        for task, (config_text, sudsy_cmd) in tasks.items():
            if task not in speed:
                continue
            row = {"id": f"{name}/{task}", "language": speed["language"], "task": task.capitalize(), "tool": speed.get("label", name), "version": version}
            if config_text is None:
                row["not_measured"] = "no mirroring preset"
            else:
                corpus_info = corpus_info or corpus(name, config, speed, work)
                row.update(corpus=corpus_info[2], files=corpus_info[0], bytes=corpus_info[1])
                (work / "sudsy.yaml").write_text(config_text, encoding="utf-8", newline="\n")
                row.update(best_of(fill(sudsy_cmd), fill(speed[task]), work, os.environ, tool_env))
            rows.append(row)
            print(json.dumps(row), flush=True)
    out = Path(args.out)
    order = list(known)
    if args.tools and out.exists():  # a partial run replaces the rows of its tools and keeps the others
        old = [r for r in json.loads(out.read_text())["rows"] if r["id"].split("/")[0] not in chosen]
        rows = sorted(old + rows, key=lambda r: order.index(r["id"].split("/")[0]) if r["id"].split("/")[0] in order else len(order))
    commit = subprocess.run(["git", "rev-parse", "--short", "HEAD"], cwd=ROOT, capture_output=True, text=True).stdout.strip()
    out.write_text(json.dumps({"runner": runner(), "runs": RUNS, "commit": commit, "rows": rows}, indent=1) + "\n", encoding="utf-8", newline="\n")


if __name__ == "__main__":
    main()
