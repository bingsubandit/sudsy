"""wrap.py META [ARGS...]: the `sudsy` a subject calls in a shim arm (arms.toml `shim`), in front of the
real binary (META/condition.json `sudsy`). Every other command passes through unchanged.

concise  check, lint, fix without --format get the real `--format concise`
full     check, lint without --format or --rule get the real `--rule` with every rule present, so the
         human report is never collapsed
noids    PROVISIONAL (no Sudsy flag): fix ids are removed from the output and `fix --apply` is refused
         as an unknown argument, as a Sudsy without fix ids would
"""

import json
import os
import pathlib
import re
import subprocess
import sys

META = pathlib.Path(sys.argv[1])
ARGS = sys.argv[2:]
CFG = json.loads((META / "condition.json").read_text())
REAL = CFG["sudsy"]
MODE = CFG.get("shim")


def run(args, **kw):
    return subprocess.run([REAL, *args], capture_output=True, text=True, **kw)


def passthrough(args):
    os.execv(REAL, [REAL, *args])


def has_opt(args, name):
    return any(a == name or a.startswith(name + "=") for a in args)


def positional(args):
    """The path arguments of check/lint/fix (options with a value skipped)."""
    out, skip = [], False
    for a in args[1:]:
        if skip:
            skip = False
        elif a in ("--format", "--analysis", "--rule", "--apply"):
            skip = True
        elif not a.startswith("-"):
            out.append(a)
    return out


def emit(r, text=None):
    sys.stdout.write(r.stdout if text is None else text)
    sys.stderr.write(r.stderr)
    sys.exit(r.returncode)


def main():
    if not ARGS or ARGS[0] not in ("check", "lint", "fix"):
        passthrough(ARGS)
    sub = ARGS[0]
    if MODE == "concise" and not has_opt(ARGS, "--format"):
        passthrough([*ARGS, "--format", "concise"])
    if (
        MODE == "full"
        and sub != "fix"
        and not has_opt(ARGS, "--format")
        and not has_opt(ARGS, "--rule")
    ):
        r = run(["lint", "--format", "json", *positional(ARGS)])
        try:
            rules = sorted({d["rule"] for d in json.loads(r.stdout)["diagnostics"]})
        except (ValueError, KeyError):
            rules = []
        passthrough([*ARGS, "--rule", ",".join(rules)] if rules else ARGS)
    if MODE == "noids":
        if has_opt(ARGS, "--apply"):
            sys.stderr.write(
                "error: unexpected argument '--apply' found\n\nUsage: sudsy fix [OPTIONS] [PATHS]...\n"
            )
            sys.exit(2)
        r = run(ARGS)
        out = re.sub(r"\bfix [a-z0-9]{6} \(", "fix (", r.stdout)
        if has_opt(ARGS, "--help") or has_opt(ARGS, "-h"):
            out = re.sub(r"\n +--apply <ID>\n(?: {10,}.*\n)*", "\n", out)
        emit(r, out)
    passthrough(ARGS)


if __name__ == "__main__":
    main()
