"""hook.py post|stop META: the hooks of the hook conditions (Claude Code hook protocol, as in the pilot).

post (PostToolUse on Edit|Write|MultiEdit): lints the edited file with the family's `hook_lint`, never
formats; clean: no output, exit 0; findings: the tool's output on stderr, exit 2 (the agent reads it in
the same turn). stop (Stop): runs the family's fix commands on the changed files, exit 0. Every call is logged to META/calls.log.
For `sudsy-hooks` this is the provisional stand-in for `sudsy hook` (plan step 5)."""
import json
import pathlib
import re
import subprocess
import sys
import time

mode, meta = sys.argv[1], pathlib.Path(sys.argv[2])
cfg = json.loads((meta / "condition.json").read_text())
run = pathlib.Path(cfg["run"])
try:
    event = json.load(sys.stdin)
except ValueError:
    event = {}


def log(what):
    with open(meta / "calls.log", "a", newline="\n") as f:
        f.write(f"{int(time.time())} hook-{mode} {what}\n")


def sh(cmd):
    return subprocess.run(cmd, shell=True, cwd=run, capture_output=True, text=True, timeout=300)


if mode == "post":
    path = pathlib.Path((event.get("tool_input") or {}).get("file_path", ""))
    if not path.is_absolute():
        path = run / path
    try:
        rel = path.resolve().relative_to(run.resolve()).as_posix()
    except ValueError:
        sys.exit(0)
    if path.suffix not in cfg["exts"] or not path.exists():
        sys.exit(0)
    log(rel)
    pkg = "./" + (str(pathlib.PurePosixPath(rel).parent) if "/" in rel else ".")
    r = sh(cfg["hook_lint"].format(file=rel, package=pkg))
    out = (r.stdout + r.stderr).strip()
    # a finding is a failing exit or a `path:line:col:` line (Sudsy exits 0 on warnings)
    if out and (r.returncode != 0 or re.search(r":\d+:\d+:", out)):
        print(out, file=sys.stderr)
        sys.exit(2)
    sys.exit(0)

if mode == "stop":
    log("fix")
    sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
    from common import fix_changed

    fix_changed(cfg["fix"], cfg["exts"], run)
    sys.exit(0)
