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
import re
import shutil
import subprocess
import sys
import tempfile
import time
from collections import Counter
from pathlib import Path

import parity_coverage as pc
import parity_format as pf

ROOT = pf.ROOT
RUNS = 5
CHUNK = 500  # files per process when a command takes `{files}`


def entries():
    """name -> (config, [speed, ...]) for every entry with a `speed` key, in file order; a list is one tool measured on
    several languages. A tool with a parity entry carries the key there; one without (oxlint, dprint) is an entry of the
    `speed` section, which no parity runner reads."""
    found = {}
    for section in ("format", "lint", "speed"):
        for name, config in pf.tools().get(section, {}).items():
            if "speed" in config:
                speed = config["speed"]
                found.setdefault(name, (config, speed if isinstance(speed, list) else [speed]))
    return found


def coverage_of(name, config):
    """The coverage run of a tool: on its entry, else on its lint entry (a tool with a format and a lint entry has the speed key on the first)."""
    return config.get("coverage") or pf.tools()["lint"][name]["coverage"]


def commands_for(command, values, paths):
    """The tool's commands with the placeholders filled; `{files}` is every corpus file, in chunks of CHUNK per process."""
    fill = lambda a: str(a).format(**values)  # noqa: E731
    if "{files}" not in command:
        return [[fill(a) for a in command]]
    i = command.index("{files}")
    return [[*map(fill, command[:i]), *paths[k:k + CHUNK], *map(fill, command[i + 1:])] for k in range(0, len(paths), CHUNK)]


def sudsy_config(name, speed, task, lint_presets):
    """(the sudsy.yaml text of the Sudsy side, None) or (None, why the task is not measured). The mirroring preset of a
    format run is the `preset` of the speed key (one name or a list), else the `sudsy.yaml` of the tool's parity folder;
    of a lint run the `lint_preset` of the key, else the preset of the tool's lint entry."""
    if task == "format":
        if "preset" in speed:
            names = [speed["preset"]] if isinstance(speed["preset"], str) else speed["preset"]
            return json.dumps({"extends": [f"preset:{n}" for n in names]}), None
        folder = ROOT / "tests/parity" / speed.get("sudsy", name) / "sudsy.yaml"
        return (folder.read_text(encoding="utf-8"), None) if folder.exists() else (None, "no mirroring preset")
    preset = speed.get("lint_preset") or lint_presets.get(name)
    return (json.dumps({"extends": [f"preset:{preset}"]}), None) if preset else (None, "no mirroring preset")


def prepare(name, config, specs, parity):
    """Builds what the entry needs, fetches its corpus and installs the tool; returns (version, prefix, exe)."""
    if "worker" in config.get("needs", []):
        pf.run(["dotnet", "build", "-c", "Release", "--nologo", "-v:q", "workers/csharp/Sudsy.Worker"], ROOT)
        os.environ.setdefault("SUDSY_CSHARP_WORKER", str(ROOT / "workers/csharp/Sudsy.Worker/bin/Release/net10.0/Sudsy.Worker.dll"))
    if any(isinstance(s["corpus"], dict) for s in specs):  # a named corpus fetches itself; a group or a coverage corpus needs the entry's
        for script in [*config.get("fetch", []), *pf.tools()["lint"].get(name, {}).get("fetch", [])]:
            pf.run(["bash", script], ROOT)
    catalog = config.get("catalog")
    version = config.get("version") or pf.catalog_version(catalog)
    prefix = parity / f"{catalog or name}-{version}"
    spec = config.get("install")
    spec = pf.tools()["format"][spec]["install"] if isinstance(spec, str) else spec
    if spec:
        pf.install(spec, catalog or name, version, prefix)
    for speed in specs:
        for file, text in speed.get("files", {}).items():  # config files the tool's command reads
            (prefix / file).write_text(text.replace("{prefix}", str(prefix)), encoding="utf-8", newline="\n")
    return version, prefix, pf.executable(prefix, spec["exe"]) if spec and "exe" in spec else ""


def corpus(name, config, speed, work):
    """Copies the speed corpus into `work/c` (`copies` times) and returns (files, bytes, text, paths below `work/c`).
    `corpus` names an entry of `corpora` in parity_tools.json (fixture sources shared by the tools of a language), is
    `{"group": <ratchet group of the entry's fixtures>}` (`fixtures`: the ungrouped ones), or `{"coverage": true}`: the
    corpus of the entry's coverage run (`text`, `copies` as in the other forms)."""
    spec = speed["corpus"]
    if isinstance(spec, str):
        spec = pf.tools()["corpora"][spec]
    if "coverage" in spec:
        found = [(rel, path, None) for rel, path in pc.corpus_files(coverage_of(name, config), Counter())]
    else:
        if "group" in spec:
            spec = {**spec, "sources": [s for s in config["fixtures"] if (s.get("ratchet") or "fixtures") == spec["group"]]}
        for script in spec.get("fetch", []):
            pf.run(["bash", script], ROOT)
        found = pf.fixtures({"fixtures": spec["sources"]}, ROOT / "tests/parity" / name, Counter())
    if not found:
        raise SystemExit(f"{name}: the speed corpus is empty")
    copies = spec.get("copies", 1)
    shutil.rmtree(work, ignore_errors=True)
    size, paths = 0, []
    for i in range(copies):
        for rel, path, _ in found:
            inside = (f"c{i + 1}/" if copies > 1 else "") + rel
            dest = work / "c" / inside
            dest.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(path, dest)
            size += path.stat().st_size
            paths.append(inside)
    return len(found) * copies, size, spec.get("text") or f"{len(found)} files", paths


def files_checked(summary):
    """The N of the `N file(s) checked` line that `sudsy format` and `sudsy lint` print last on stderr; None when absent."""
    m = re.findall(r"(\d+) file\(s\) checked", summary)
    return int(m[-1]) if m else None


def check_sudsy_run(command, cwd, code, summary):
    """A Sudsy run is a timing only when it exited 0 or 1 (findings or differences; a negative code is a crash) and
    checked files: a run that checks none is a wrong corpus path, folder or preset, and its time means nothing."""
    where = f"{' '.join(map(str, command))} in {cwd}"
    if code < 0 or code > 1:
        raise SystemExit(f"{where}: exit {code}")
    if not files_checked(summary):
        raise SystemExit(f"{where}: Sudsy checked 0 files")


def measure(command, cwd, env, capture=False):
    """One run in a new process: (wall seconds, peak resident MB of the process and the children it waited for, exit
    code, stderr text when `capture`)."""
    err = tempfile.TemporaryFile() if capture else subprocess.DEVNULL
    start = time.perf_counter()
    p = subprocess.Popen(command, cwd=cwd, env=env, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=err)
    _, status, usage = os.wait4(p.pid, 0)
    seconds = time.perf_counter() - start
    p.returncode = os.waitstatus_to_exitcode(status)
    if p.returncode in (126, 127):
        raise SystemExit(f"{' '.join(map(str, command))}: cannot run")
    text = ""
    if capture:
        err.seek(0)
        text = err.read().decode("utf-8", "replace")
        err.close()
    return seconds, usage.ru_maxrss / 1024, p.returncode, text  # ru_maxrss is KiB on Linux


def measure_all(commands, cwd, env, capture=False):
    """The commands one after the other (a tool given `{files}` runs once per chunk): the seconds add up, the memory is the peak."""
    runs = [measure(c, cwd, env, capture) for c in commands]
    return sum(r[0] for r in runs), max(r[1] for r in runs), max(r[2] for r in runs), runs[-1][3]


def best_of(sudsy_cmd, tool_cmds, sudsy_cwd, tool_cwd, sudsy_env, tool_env):
    """Alternates Sudsy and the tool; the best run of each by wall time, as {"s", "mb"}."""
    best = {}
    for _ in range(RUNS):
        for key, commands, cwd, env in (("sudsy", [sudsy_cmd], sudsy_cwd, sudsy_env), ("other", tool_cmds, tool_cwd, tool_env)):
            seconds, mb, code, summary = measure_all(commands, cwd, env, capture=key == "sudsy")
            if key == "sudsy":
                check_sudsy_run(commands[0], cwd, code, summary)
            if key not in best or seconds < best[key]["s"]:
                best[key] = {"s": round(seconds, 3), "mb": round(mb)}
    return best


def runner():
    cpu = next((l.split(":", 1)[1].strip() for l in open("/proc/cpuinfo") if l.startswith(("model name", "Model name"))), "")
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
        config, specs = known[name]
        if not any(task in s for s in specs for task in ("format", "lint")):  # nothing to run: no install, only the reasons
            version = config.get("version") or (pf.catalog_version(config["catalog"]) if "catalog" in config else "")
            prefix = exe = ""
        else:
            try:
                version, prefix, exe = prepare(name, config, specs, parity)
            except (SystemExit, Exception) as e:  # an install or fetch that fails leaves the tool's rows "not measured", with the error
                print(f"{name}: {e}", file=sys.stderr, flush=True)
                version, prefix, exe = config.get("version") or "?", "", ""
                specs = [{**s, "not_measured": {t: f"setup failed: {str(e).strip().splitlines()[-1][:150]}" for t in ("format", "lint") if t in s}} for s in specs]
        values = {"prefix": prefix, "exe": exe, "python": sys.executable, "root": ROOT, "sudsy": args.sudsy}
        fill = lambda cmd: [str(a).format(**values) for a in cmd]  # noqa: E731
        tool_env = {**os.environ, **{k: v.format(prefix=prefix) for k, v in config.get("env", {}).items()}}
        for i, speed in enumerate(specs):
            work = parity / f"{name}-speed{i or ''}"
            corpus_info = None
            for task in ("format", "lint"):
                if task not in speed and task not in speed.get("not_measured", {}):
                    continue
                row = {"id": f"{name}/{speed['language']}/{task}", "language": speed["language"], "task": task.capitalize(), "tool": speed.get("label", name), "version": version}
                reason = speed.get("not_measured", {}).get(task)  # a task the tool cannot run here, with the reason
                config_text = None
                if not reason:
                    config_text, reason = sudsy_config(name, speed, task, lint_presets)
                if config_text is None:
                    row["not_measured"] = reason
                else:
                  try:
                    corpus_info = corpus_info or corpus(name, config, speed, work)
                    row.update(corpus=corpus_info[2], files=corpus_info[0], bytes=corpus_info[1])
                    (work / "sudsy.yaml").write_text(config_text, encoding="utf-8", newline="\n")  # above the corpus, so it is no input
                    command = speed[task]
                    coverage = command == "coverage"  # the command of the entry's coverage run, on the files of its corpus
                    env = {**tool_env, **{k: v.format(prefix=prefix) for k, v in speed.get("env", {}).items()}}
                    if coverage:
                        cov = coverage_of(name, config)
                        command = cov["command"]
                        for file, text in cov.get("files", {}).items():
                            (work / file).write_text(text.replace("{prefix}", str(prefix)), encoding="utf-8", newline="\n")
                        env = {**env, **{k: v.format(prefix=prefix) for k, v in cov.get("env", {}).items()}}
                    cwd = work if coverage else work / "c"  # a coverage command names its config files relative to `work`
                    paths = ["c/" + p for p in corpus_info[3]] if coverage else corpus_info[3]
                    cmds = commands_for(command, {**values, "work": work}, paths)
                    sudsy_cmd = ["{sudsy}", "format", "--check", "."] if task == "format" else ["{sudsy}", "lint", "."]
                    row.update(best_of(fill(sudsy_cmd), cmds, work / "c", cwd, os.environ, env))
                  except (SystemExit, Exception) as e:  # a run that fails is a row "not measured", with the error
                    print(f"{name}/{speed['language']}/{task}: {e}", file=sys.stderr, flush=True)
                    row = {k: v for k, v in row.items() if k not in ("corpus", "files", "bytes", "sudsy", "other")}
                    row["not_measured"] = f"run failed: {str(e).strip().splitlines()[-1][:150]}"
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
