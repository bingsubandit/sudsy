"""setup_run.py VERSION TASK ARM REP DIR [SUDSY]: a fresh run directory DIR/TASK-ARM-REP (the repository at
the task's start, one git commit tagged bench-base, nothing of the task's solution or hidden tests) and
its META directory next to it (prompt, arm, hooks, the `sudsy` on PATH, hidden files, a copy of the start
tree)."""

import pathlib
import shutil
import subprocess
import sys

import yaml

import arms
from common import (
    CACHE,
    HERE,
    apply_edits,
    bare,
    git,
    load_task,
    repos,
    run_env,
    sh,
    task_dir,
    write_json,
)
from fetch import export

GIT_ID = ["-c", "user.name=bench", "-c", "user.email=bench@example.com"]


def start_rev(task):
    if task.get("commit"):
        return task["commit"] + "^"
    return task.get("base") or repos()["repo"][task["repo"]]["pin"]


def commit_files(repo, commit):
    """Files of a commit: [(status, path)]."""
    out = git(
        bare(repo),
        "diff-tree",
        "--no-commit-id",
        "-r",
        "--name-status",
        "--no-renames",
        f"{commit}^",
        commit,
    )
    return [tuple(l.split("\t", 1)) for l in out.splitlines() if l]


def show(repo, rev, path):
    return subprocess.run(
        ["git", "show", f"{rev}:{path}"],
        cwd=bare(repo),
        capture_output=True,
        check=True,
    ).stdout


def materialize(task, meta):
    """meta/hidden: the files the verifier puts in place before the tests (commit tasks: the commit's
    versions of `hidden`; others: the task's hidden/ folder). meta/reference: the reference solution."""
    hidden, ref = meta / "hidden", meta / "reference"
    hidden.mkdir()
    ref.mkdir()
    if task.get("commit"):
        for p in task.get("hidden", []):
            (hidden / p).parent.mkdir(parents=True, exist_ok=True)
            (hidden / p).write_bytes(show(task["repo"], task["commit"], p))
        deleted = []
        for st, p in commit_files(task["repo"], task["commit"]):
            if st == "D":
                deleted.append(p)
                continue
            (ref / p).parent.mkdir(parents=True, exist_ok=True)
            (ref / p).write_bytes(show(task["repo"], task["commit"], p))
        (meta / "reference.deleted").write_text("\n".join(deleted))
    else:
        src = task_dir(task["version"], task["id"])
        if (src / "hidden").exists():
            shutil.copytree(src / "hidden", hidden, dirs_exist_ok=True)
        for p in src.glob("solution*.patch"):
            shutil.copy(p, meta / p.name)


def merge(a, b):
    """b merged into a: dicts recursively, lists extended by the items they lack, other values replaced."""
    for k, v in b.items():
        if isinstance(v, dict) and isinstance(a.get(k), dict):
            merge(a[k], v)
        elif isinstance(v, list) and isinstance(a.get(k), list):
            a[k] = a[k] + [x for x in v if x not in a[k]]
        else:
            a[k] = v
    return a


def add_sudsy_parts(root, parts):
    """Write each part's `files` ({path: text}) and merge its `config` (a sudsy.yaml fragment: YAML text or a table)
    into root/sudsy.yaml."""
    cfg_path = pathlib.Path(root) / "sudsy.yaml"
    cfg = (yaml.safe_load(cfg_path.read_text()) or {}) if cfg_path.exists() else {}
    for part in parts:
        for path, text in (part.get("files") or {}).items():
            p = pathlib.Path(root) / path
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text(text)
        frag = part.get("config") or {}
        merge(cfg, yaml.safe_load(frag) if isinstance(frag, str) else frag)
    with open(cfg_path, "w", newline="\n") as f:
        yaml.safe_dump(cfg, f, sort_keys=False)


def prepare_tree(task, arm, run, sudsy):
    """Everything the start tree needs before its commit: the seed, the repo's setup, node_modules, the
    policy text, for the sudsy arms their sudsy.yaml (presets or the matched config, the task's
    project rules, its policy rule unless the arm leaves it out) and the tree formatted by Sudsy, then
    the task's `seed_edits`."""
    repo = {**repos()["repo"][task["repo"]], "name": task["repo"]}
    env = run_env(repo)
    seed = task_dir(task["version"], task["id"]) / "seed.patch"
    if seed.exists():
        r = sh(["git", "apply", str(seed)], cwd=run)
        if r.returncode != 0:
            raise RuntimeError(f"seed.patch does not apply: {r.stderr}")
    for c in repo.get("setup", []):
        r = sh(c, cwd=run, env=env)
        if r.returncode != 0:
            raise RuntimeError(f"setup {c!r} failed: {r.stderr}")
    if "node" in repo:
        (run / "node_modules").symlink_to(
            CACHE / "node" / task["repo"] / "node_modules"
        )
    policy = task.get("policy")
    if policy:
        # stated in both arms of E10, in the file Claude Code reads as project instructions
        with open(run / "CLAUDE.md", "a", newline="\n") as f:
            f.write(policy["text"].strip() + "\n")
    a = arms.arm(arm)
    if a["family"] == "sudsy":
        sudsy_tree(task, a, repo, run, sudsy, env, policy)
    # seeded edits go in last, on the tree as each arm's tools left it, so they stay unformatted
    apply_edits(run, task.get("seed_edits", []))


def sudsy_tree(task, a, repo, run, sudsy, env, policy):
    if a.get("config") == "matched":
        shutil.copy(HERE / "configs" / f"{task['repo']}.sudsy.yaml", run / "sudsy.yaml")
    elif not (run / "sudsy.yaml").exists():
        r = sh(
            [sudsy, "init", "-y", *repo.get("sudsy_init", "").split()], cwd=run, env=env
        )
        if r.returncode != 0:
            raise RuntimeError(f"sudsy init failed: {r.stderr}")
    parts = [{"config": repo["sudsy_config"]}] if repo.get("sudsy_config") else []
    if task.get("project"):
        parts.append(task["project"])
    if policy and a.get("policy", True):
        parts.append(policy)
    if parts:
        add_sudsy_parts(run, parts)
    # a project that uses Sudsy is formatted by it, as the native start trees are by their tools
    sh([sudsy, "format"], cwd=run, env=env)


def write_shim(meta, arm, sudsy):
    """META/bin/sudsy: the binary, or for a shim arm wrap.py in front of it."""
    b = meta / "bin"
    b.mkdir()
    a = arms.arm(arm)
    if a["family"] != "sudsy":
        return
    if a.get("shim"):
        with open(b / "sudsy", "w", newline="\n") as f:
            f.write(f'#!/bin/sh\nexec python3 -I {HERE / "wrap.py"} {meta} "$@"\n')
        (b / "sudsy").chmod(0o755)
    else:
        (b / "sudsy").symlink_to(sudsy)


def setup(version, task_id, arm, rep, root, sudsy):
    task = load_task(version, task_id)
    root = pathlib.Path(root)
    run = root / f"{task_id}-{arm}-{rep}"
    meta = root / f"{task_id}-{arm}-{rep}.meta"
    for d in (run, meta):
        shutil.rmtree(d, ignore_errors=True)
    meta.mkdir(parents=True)
    subprocess.run(["git", "init", "-q", "-b", "main", str(run)], check=True)
    export(task["repo"], start_rev(task), run)
    prepare_tree(task, arm, run, sudsy)
    git(run, *GIT_ID, "add", "-A")
    git(run, *GIT_ID, "commit", "-q", "--no-verify", "-m", f"start of {task_id}")
    git(run, "tag", "bench-base")
    git(run, "config", "user.name", "bench")
    git(run, "config", "user.email", "bench@example.com")
    # the verifier's copy of the start tree (base findings)
    shutil.copytree(
        run, meta / "base", symlinks=True, ignore=shutil.ignore_patterns(".git")
    )
    materialize(task, meta)
    fam = arms.family(task["repo"], arm)
    write_shim(meta, arm, sudsy)
    write_json(
        meta / "condition.json",
        {
            "run": str(run),
            "cond": arm,
            "shim": arms.arm(arm).get("shim"),
            "task": task_id,
            "rep": rep,
            "version": version,
            "sudsy": sudsy,
            "hook_lint": fam["hook_lint"],
            "exts": fam["exts"],
            "fix": fam["hook_fix"],
        },
    )
    write_json(meta / "settings.json", arms.settings(arm, meta))
    (meta / "TASK.md").write_text(
        task["text"].strip() + "\n\n" + arms.instructions(task["repo"], arm)
    )
    (meta / "calls.log").write_text("")
    return run, meta


if __name__ == "__main__":
    v, t, a, rep, d = sys.argv[1:6]
    print(*setup(v, t, a, int(rep), d, sys.argv[6] if len(sys.argv) > 6 else "sudsy"))
