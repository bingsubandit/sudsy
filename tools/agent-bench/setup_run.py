"""setup_run.py VERSION TASK COND REP DIR: a fresh run directory DIR/TASK-COND-REP (the repository at the
task's start, one git commit tagged bench-base, nothing of the task's solution or hidden tests) and its
META directory next to it (prompt, condition, hooks, hidden files, a copy of the start tree)."""
import pathlib
import shutil
import subprocess
import sys

import conditions
from common import CACHE, bare, git, load_task, repos, run_env, sh, task_dir, write_json
from fetch import export

GIT_ID = ["-c", "user.name=bench", "-c", "user.email=bench@example.com"]


def start_rev(task):
    if task.get("commit"):
        return task["commit"] + "^"
    return task.get("base") or repos()["repo"][task["repo"]]["pin"]


def commit_files(repo, commit):
    """Files of a commit: [(status, path)]."""
    out = git(bare(repo), "diff-tree", "--no-commit-id", "-r", "--name-status", "--no-renames", f"{commit}^", commit)
    return [tuple(l.split("\t", 1)) for l in out.splitlines() if l]


def show(repo, rev, path):
    return subprocess.run(["git", "show", f"{rev}:{path}"], cwd=bare(repo), capture_output=True, check=True).stdout


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


def prepare_tree(task, cond, run, sudsy):
    """Everything the start tree needs before its commit: the seed, the repo's setup, node_modules, the
    sudsy config of the sudsy conditions."""
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
        (run / "node_modules").symlink_to(CACHE / "node" / task["repo"] / "node_modules")
    if conditions.family_name(cond) == "sudsy" and not (run / "sudsy.yaml").exists():
        r = sh([sudsy, "init", "-y", *repo.get("sudsy_init", "").split()], cwd=run, env=env)
        if r.returncode != 0:
            raise RuntimeError(f"sudsy init failed: {r.stderr}")
        # a project that uses Sudsy is formatted by it, as the native start trees are by their tools
        sh([sudsy, "format"], cwd=run, env=env)


def setup(version, task_id, cond, rep, root, sudsy):
    task = load_task(version, task_id)
    root = pathlib.Path(root)
    run = root / f"{task_id}-{cond}-{rep}"
    meta = root / f"{task_id}-{cond}-{rep}.meta"
    for d in (run, meta):
        shutil.rmtree(d, ignore_errors=True)
    meta.mkdir(parents=True)
    subprocess.run(["git", "init", "-q", "-b", "main", str(run)], check=True)
    export(task["repo"], start_rev(task), run)
    prepare_tree(task, cond, run, sudsy)
    git(run, *GIT_ID, "add", "-A")
    git(run, *GIT_ID, "commit", "-q", "--no-verify", "-m", f"start of {task_id}")
    git(run, "tag", "bench-base")
    git(run, "config", "user.name", "bench")
    git(run, "config", "user.email", "bench@example.com")
    # the verifier's copy of the start tree (base findings)
    base = meta / "base"
    shutil.copytree(run, base, symlinks=True, ignore=shutil.ignore_patterns(".git"))
    materialize(task, meta)
    fam = conditions.family(task["repo"], cond)
    write_json(meta / "condition.json", {"run": str(run), "cond": cond, "task": task_id, "rep": rep, "version": version, "sudsy": sudsy,
                                         "hook_lint": fam["hook_lint"], "exts": fam["exts"], "fix": fam["fix"]})
    write_json(meta / "settings.json", conditions.settings(cond, meta))
    prompt = task["text"].strip() + "\n\n" + conditions.instructions(task["repo"], cond)
    (meta / "TASK.md").write_text(prompt)
    (meta / "calls.log").write_text("")
    return run, meta


if __name__ == "__main__":
    v, t, c, rep, d = sys.argv[1:6]
    sudsy = sys.argv[6] if len(sys.argv) > 6 else "sudsy"
    print(*setup(v, t, c, int(rep), d, sudsy))
