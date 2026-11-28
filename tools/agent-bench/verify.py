"""verify.py RUN META SUDSY: the verifier of one run, one JSON line.

A run succeeds when all pass: the task's tests (after its hidden files are put in place), the task's own
checks and probes, nothing changed outside the task's scope, and, with the condition's own tools, no new
lint finding on the changed files and every changed file formatted. The other family's counts are
recorded too (mistake counters), never part of success. A finding is new unless the same (path, rule,
line text) exists in the start tree as often."""
import collections
import json
import pathlib
import shlex
import shutil
import sys

import conditions
from common import changed_files, load_task, matches, repos, run_env, sh


def line_text(root, path, line):
    try:
        lines = (root / path).read_text(errors="replace").splitlines()
        return lines[line - 1].strip() if 0 < line <= len(lines) else ""
    except OSError:
        return ""


def parse(kind, r, root):
    """[(path, line, rule)] from a lint tool's output, or None when the tool failed."""
    rel = lambda p: pathlib.Path(p).resolve().relative_to(root.resolve()).as_posix() if pathlib.Path(p).is_absolute() else p.removeprefix("./")
    try:
        if kind == "ruff":
            return [(rel(d["filename"]), d["location"]["row"], d.get("code") or "syntax") for d in json.loads(r.stdout or "[]")]
        if kind == "eslint":
            return [(rel(f["filePath"]), m.get("line", 0), m.get("ruleId") or "syntax") for f in json.loads(r.stdout or "[]") for m in f["messages"]]
        if kind == "sudsy":
            return [(d["path"], d["range"]["start"]["line"], d["rule"]) for d in json.loads(r.stdout)["diagnostics"]]
        if kind == "govet":
            out = []
            for l in r.stderr.splitlines():
                parts = l.split(":", 3)
                if len(parts) == 4 and parts[1].isdigit():
                    out.append((rel(parts[0]), int(parts[1]), "vet:" + parts[3].strip()))
            if r.returncode not in (0, 1) and not out:
                return None
            return out
    except (ValueError, KeyError):
        return None
    raise ValueError(kind)


def lint(fam, root, files, env):
    files = [f for f in files if pathlib.Path(f).suffix in fam["exts"] and (root / f).exists()]
    if not files:
        return []
    pkgs = sorted({"./" + str(pathlib.PurePosixPath(f).parent) for f in files})
    cmd = fam["lint"]["cmd"].format(files=" ".join(shlex.quote(f) for f in files), packages=" ".join(pkgs))
    found = parse(fam["lint"]["parse"], sh(cmd, cwd=root, env=env), root)
    if found is None:
        return None
    return [(p, ln, rule, line_text(root, p, ln)) for p, ln, rule in found if p in files]


def new_findings(fam, run, base, files, env):
    end, start = lint(fam, run, files, env), lint(fam, base, files, env)
    if end is None or start is None:
        return None
    left = collections.Counter((p, rule, text) for p, _, rule, text in start)
    new = []
    for p, ln, rule, text in end:
        if left[(p, rule, text)] > 0:
            left[(p, rule, text)] -= 1
        else:
            new.append(f"{p}:{ln}: {rule}")
    return new


def is_formatted(rule, root, f, env):
    r = sh(rule["cmd"].format(file=shlex.quote(f)), cwd=root, env=env)
    return r.returncode == 0 and not (rule.get("output_fails") and r.stdout.strip())


def unformatted(fam, run, base, files, env):
    """Changed files that the family's formatter would change, among those it formatted at the start
    (and new files): like findings, only formatting the run broke counts."""
    bad = []
    for f in files:
        if not (run / f).exists():
            continue
        rule = next((c for c in fam["format_check"] if pathlib.Path(f).suffix in c["exts"]), None)
        if rule is None or ((base / f).exists() and not is_formatted(rule, base, f, env)):
            continue
        if not is_formatted(rule, run, f, env):
            bad.append(f)
    return bad


def put_hidden(run, meta):
    hidden = meta / "hidden"
    for p in hidden.rglob("*"):
        if p.is_file():
            dest = run / p.relative_to(hidden)
            dest.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy(p, dest)


def probes(task, cond, run, base, env):
    """Policy tasks: each probe file is written into the run and the start copy, linted with the condition's
    tools and removed; `expect` is `finding` (more findings than with the start config: the new rule reports
    it) or `clean` (no more). A probe without `content` lints an existing file."""
    fam = conditions.family(task["repo"], cond)
    failed = []
    for pr in task.get("probes", []):
        counts = []
        for root in (run, base):
            p = root / pr["path"]
            if "content" in pr:
                p.parent.mkdir(parents=True, exist_ok=True)
                p.write_text(pr["content"])
            found = lint(fam, root, [pr["path"]], env)
            if "content" in pr:
                p.unlink()
            counts.append(None if found is None else len(found))
        end, start = counts
        # the probe's findings at the end against the same file with the start config: the new rule adds some
        hit = end is not None and start is not None and end > start
        if hit != (pr["expect"] == "finding") or end is None:
            failed.append(pr["path"])
    return failed


def verify(run, meta, sudsy):
    run, meta = pathlib.Path(run), pathlib.Path(meta)
    c = json.loads((meta / "condition.json").read_text())
    task = load_task(c["version"], c["task"])
    repo = {**repos()["repo"][task["repo"]], "name": task["repo"]}
    env = run_env(repo, extra_path=[pathlib.Path(sudsy).parent])
    files = changed_files(run)
    res = {"changed_files": files, "out_of_scope": [f for f in files if not matches(f, task["scope"])]}
    for famname in ("native", "sudsy"):
        fam = conditions.family(task["repo"], famname)
        res[f"{famname}_new_findings"] = new_findings(fam, run, meta / "base", files, env)
        res[f"{famname}_unformatted"] = unformatted(fam, run, meta / "base", files, env)
    res["probes_failed"] = probes(task, c["cond"], run, meta / "base", env)
    # lint-debt tasks: these files must end with no finding at all of the condition's tools
    own = conditions.family(task["repo"], c["cond"])
    res["not_clean"] = [f for f in task.get("clean", []) if lint(own, run, [f], env) != []]
    put_hidden(run, meta)
    t = sh(task["tests"], cwd=run, env=env, timeout=900)
    res["tests_pass"] = t.returncode == 0
    res["tests_tail"] = (t.stdout + t.stderr)[-600:] if t.returncode else ""
    res["checks_failed"] = [cmd for cmd in task.get("checks", []) if sh(cmd, cwd=run, env=env).returncode != 0]
    fam = conditions.family_name(c["cond"])
    res["lint_clean"] = res[f"{fam}_new_findings"] == []
    res["formatted"] = res[f"{fam}_unformatted"] == []
    res["success"] = (res["tests_pass"] and not res["checks_failed"] and not res["probes_failed"] and not res["not_clean"]
                      and not res["out_of_scope"] and res["lint_clean"] and res["formatted"] and bool(files))
    return res


if __name__ == "__main__":
    print(json.dumps(verify(*sys.argv[1:4])))
