"""The conditions: which tools a run gets, its instruction block (one template), its allowlist and hooks.

native        the repository's own tools, as its scripts call them
native-hooks  the same tools behind the post-edit and stop hooks
sudsy         `sudsy check` / `sudsy fix`
sudsy-hooks   Sudsy behind the hooks; PROVISIONAL until `sudsy hook` exists (plan step 5): hook.py runs
              `sudsy lint FILE` after an edit and `sudsy fix` at stop, the protocol of the pilot's hooks
"""
import json

from common import HERE, repos

CONDITIONS = ["native", "native-hooks", "sudsy", "sudsy-hooks"]
# the name a condition's results carry; a provisional implementation is marked in it
RESULT_NAME = {"sudsy-hooks": "sudsy-hooks-provisional"}
READ_ONLY = ["git diff", "git status", "ls", "cat", "head", "tail", "wc", "grep", "sed", "mkdir"]


def family_name(cond):
    return cond.split("-")[0]


def family(repo_name, cond):
    """The tools of a condition's family for one repo: check, fix, lint, format_check (a list of
    {cmd, exts, output_fails}), hook_lint, exts."""
    cfg = repos()
    r = cfg["repo"][repo_name]
    if family_name(cond) == "native":
        f = dict(r["native"])
        f["format_check"] = [{**f["format_check"], "exts": f["exts"]}]
        return f
    f = dict(cfg["sudsy"])
    extra = r.get("sudsy_extra", {})
    f["check"] = f["check"] + extra.get("check", [])
    f["fix"] = f["fix"] + extra.get("fix", [])
    f["format_check"] = ([extra["format_check"]] if "format_check" in extra else []) + [{**f["format_check"], "exts": f["exts"]}]
    return f


def prefix(cmd):
    """The allowlist prefix of a command: the program and its subcommands, without options and paths
    (`black --check .` -> `black`, `python -m pytest` -> `python -m pytest`)."""
    out, toks = [], cmd.split()
    i = 0
    while i < len(toks):
        t = toks[i]
        if t == "-m" and i + 1 < len(toks):
            out += toks[i:i + 2]
            i += 2
            continue
        if t.startswith("-") or t == "." or t.startswith("./"):
            break
        out.append(t)
        i += 1
    return " ".join(out)


def allowed_commands(repo_name, cond):
    f = family(repo_name, cond)
    test = repos()["repo"][repo_name]["test"]
    seen = []
    for c in f["check"] + f["fix"] + [test]:
        p = prefix(c)
        if p not in seen:
            seen.append(p)
    return seen


def allowed_tools(repo_name, cond):
    cmds = allowed_commands(repo_name, cond) + READ_ONLY
    return ",".join(["Read", "Edit", "Write", "Glob", "Grep"] + [f"Bash({c}*)" for c in cmds])


def code_list(cmds):
    return ", ".join(f"`{c}`" for c in cmds)


def instructions(repo_name, cond):
    """The instruction block: one template, only the tool names differ between conditions."""
    text = (HERE / "instructions.md").read_text()
    body, hook_line = text.split("<!-- hooks", 1)
    hook_line = hook_line.split("\n", 1)[1].strip()
    f = family(repo_name, cond)
    vals = {
        "check": code_list(f["check"]),
        "fix": code_list(f["fix"]),
        "test": code_list([repos()["repo"][repo_name]["test"]]),
        "allowed": code_list(allowed_commands(repo_name, cond) + READ_ONLY),
        "hook_lint": f["hook_lint"].replace(" {file}", " FILE").replace(" {package}", " PACKAGE"),
    }
    vals["hooks"] = ("\n" + hook_line.format(**vals) + "\n") if cond.endswith("-hooks") else ""
    return body.format(**vals).rstrip() + "\n"


def settings(cond, meta):
    """The --settings JSON of a run: the hooks of the hook conditions, nothing otherwise."""
    if not cond.endswith("-hooks"):
        return {}
    hook = f"python3 -I {HERE / 'hook.py'}"
    return {"hooks": {
        "PostToolUse": [{"matcher": "Edit|Write|MultiEdit", "hooks": [{"type": "command", "command": f"{hook} post {meta}"}]}],
        "Stop": [{"hooks": [{"type": "command", "command": f"{hook} stop {meta}"}]}],
    }}


if __name__ == "__main__":
    import sys
    repo, cond = sys.argv[1], sys.argv[2]
    print(instructions(repo, cond))
    print(allowed_tools(repo, cond))
    print(json.dumps(settings(cond, "META")))
