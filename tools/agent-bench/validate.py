"""validate.py --sudsy PATH [--version v1] [TASK...]: checks every task without a subject, per family
(native, sudsy): the setup works; on the untouched start tree the task's own signal fails (its tests
with the hidden files in place, its checks, probes or clean files); the reference solution (the task's
commit, or solution[.<family>].patch) passes the whole verifier. One line per task; exit 1 when one fails."""
import argparse
import json
import shutil
import sys

from common import sh, task_ids, work_root
from setup_run import setup
from verify import verify


def apply_reference(run, meta, fam):
    for name in (f"solution.{fam}.patch", "solution.patch"):
        if (meta / name).exists():
            r = sh(["git", "apply", str(meta / name)], cwd=run)
            if r.returncode != 0:
                raise RuntimeError(f"{name} does not apply: {r.stderr}")
            return
    ref = meta / "reference"
    for p in ref.rglob("*"):
        if p.is_file():
            dest = run / p.relative_to(ref)
            dest.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy(p, dest)
    deleted = meta / "reference.deleted"
    for p in deleted.read_text().split("\n") if deleted.exists() else []:
        if p and (run / p).exists():
            (run / p).unlink()


def fails_untouched(v):
    return not v["tests_pass"] or bool(v["checks_failed"] or v["probes_failed"] or v["not_clean"])


def validate(version, task, sudsy, root):
    out = []
    for fam in ("native", "sudsy"):
        run, meta = setup(version, task, fam, 0, root, sudsy)
        before = verify(run, meta, sudsy)
        run, meta = setup(version, task, fam, 0, root, sudsy)
        apply_reference(run, meta, fam)
        after = verify(run, meta, sudsy)
        ok = fails_untouched(before) and after["success"]
        note = "" if ok else " " + json.dumps({"untouched_fails": fails_untouched(before), "reference": after})
        out.append(f"{fam}={'ok' if ok else 'FAIL'}{note}")
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--version", default="v1")
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
