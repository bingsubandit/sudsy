"""validate.py --sudsy PATH [--version v1] [TASK...]: checks every task without a subject, per start
tree its pairs use (sudsy; native and sudsy-matched for B; sudsy-norule for E10): the setup works; on the
untouched start tree the task's own signal fails (its tests with the hidden files in place, its checks
or clean files); the reference solution (the task's commit, or solution[.<family>].patch) passes the
whole verifier. One line per task; exit 1 when one fails."""

import argparse
import json
import pathlib
import shutil
import sys

import arms
from common import (
    apply_edits,
    fix_changed,
    load_task,
    repos,
    run_env,
    sh,
    task_ids,
    work_root,
)
from setup_run import setup
from verify import verify


def apply_reference(run, meta, fam, task):
    for key in (f"solution_edits_{fam}", "solution_edits"):
        if key in task:
            apply_edits(run, task[key])
            return
    for name in (f"solution.{fam}.patch", "solution.patch"):
        if (meta / name).exists():
            r = sh(["git", "apply", str(meta / name)], cwd=run)
            if r.returncode != 0:
                raise RuntimeError(f"{name} does not apply: {r.stderr}")
            return
    ref = meta / "reference"
    for p in ref.rglob("*"):
        # the commit's test files are the hidden tests the verifier puts in place after linting
        if p.is_file() and p.relative_to(ref).as_posix() not in task.get("hidden", []):
            dest = run / p.relative_to(ref)
            dest.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy(p, dest)
    deleted = meta / "reference.deleted"
    for p in deleted.read_text().split("\n") if deleted.exists() else []:
        if p and (run / p).exists():
            (run / p).unlink()


def fails_untouched(v):
    return not v["tests_pass"] or bool(v["checks_failed"] or v["not_clean"])


def arms_to_check(task):
    """One arm per distinct start tree the task's pairs use (shims and hooks do not change the tree)."""
    out = ["sudsy"]
    ps = set(task.get("pairs", []))
    if ps & {"B", "B-hooks"}:
        out += ["native", "sudsy-matched"]
    if "E10" in ps:
        out.append("sudsy-norule")
    return out


def validate(version, task_id, sudsy, root):
    out = []
    task = load_task(version, task_id)
    unknown = set(task.get("pairs", [])) - set(arms.pairs())
    if unknown or not task.get("pairs"):
        return [f"ERROR pairs {sorted(unknown) or 'missing'}"]
    repo = task["repo"]
    env = run_env({**repos()["repo"][repo], "name": repo}, [pathlib.Path(sudsy).parent])
    for arm in arms_to_check(task):
        fam = arms.family_name(arm)
        run, meta = setup(version, task_id, arm, 0, root, sudsy)
        before = verify(run, meta, sudsy)
        run, meta = setup(version, task_id, arm, 0, root, sudsy)
        apply_reference(run, meta, fam, task)
        # the reference is in the upstream style; format it as a subject's fix command would
        tools = arms.family(repo, arm)
        fix_changed(tools["hook_fix"], tools["exts"], run, env)
        after = verify(run, meta, sudsy)
        ok = fails_untouched(before) and after["success"]
        note = (
            ""
            if ok
            else " "
            + json.dumps(
                {"untouched_fails": fails_untouched(before), "reference": after}
            )
        )
        out.append(f"{arm}={'ok' if ok else 'FAIL'}{note}")
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--version", default="v2")
    ap.add_argument("--sudsy", required=True)
    ap.add_argument("tasks", nargs="*")
    a = ap.parse_args()
    root = work_root() / "validate"
    bad = 0
    for t in a.tasks or task_ids(a.version):
        try:
            res = validate(a.version, t, a.sudsy, root)
        except Exception as e:  # noqa: BLE001 - report and go on with the next task
            res = [f"ERROR {e}"]
        bad += sum(not r.split("=", 1)[-1].startswith("ok") for r in res)
        print(t, *res, sep="  ", flush=True)
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
