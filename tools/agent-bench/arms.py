"""The arms (arms.toml) and pairs (pairs.toml): which tools a run gets, its instruction block (one
template), its allowlist, hooks and `sudsy` shim."""

import json
import re

from common import HERE, load_toml, repos

RUNNERS = {"python", "npm", "npx", "go"}
READ_ONLY = [
    "git diff",
    "git status",
    "ls",
    "cat",
    "head",
    "tail",
    "wc",
    "grep",
    "sed",
    "mkdir",
]


def arms():
    return load_toml(HERE / "arms.toml")["arm"]


def pairs():
    return load_toml(HERE / "pairs.toml")["pair"]


def arm(name):
    a = arms().get(name)
    if a is None:
        raise SystemExit(f"unknown arm {name}; known: {', '.join(arms())}")
    return a


def family_name(name):
    return arm(name)["family"]


def family(repo_name, name):
    """The tools of an arm for one repo: check, fix, hook_fix, lint, format_check (a list of
    {cmd, exts, output_fails}), hook_lint, exts. Family names (native, sudsy) work as arms too."""
    cfg = repos()
    r = cfg["repo"][repo_name]
    a = arms().get(name, {"family": name})
    if a["family"] == "native":
        f = dict(r["native"])
        f["format_check"] = (
            [{**f["format_check"], "exts": f["exts"]}] if "format_check" in f else []
        )
    else:
        f = dict(cfg["sudsy"])
        extra = r.get("sudsy_extra", {})
        f["check"] = f["check"] + extra.get("check", [])
        f["fix"] = f["fix"] + extra.get("fix", [])
        f["hook_fix"] = f["hook_fix"] + extra.get("fix", [])
        f["format_check"] = (
            [extra["format_check"]] if "format_check" in extra else []
        ) + [{**f["format_check"], "exts": f["exts"]}]
        for k in ("check", "fix"):
            if k in a:
                f[k] = a[k] + extra.get(k, [])
    f.setdefault("hook_fix", f["fix"])
    return f


def prefix(cmd):
    """The allowlist prefix of a command: a checker's program name, so its help and rule docs are
    allowed too (`ruff check` -> `ruff`, `sudsy fix` -> `sudsy`); for a runner (python, npm, npx, go)
    the runner and its subcommands without options and paths (`python -m pytest`, `go vet`)."""
    toks = cmd.split()
    if toks[0] not in RUNNERS:
        return toks[0]
    out, i = [], 0
    while i < len(toks):
        t = toks[i]
        if t == "-m" and i + 1 < len(toks):
            out += toks[i : i + 2]
            i += 2
            continue
        if t.startswith(("-", "./")) or t == ".":
            break
        out.append(t)
        i += 1
    return " ".join(out)


def allowed_commands(repo_name, name):
    f = family(repo_name, name)
    seen = []
    for c in f["check"] + f["fix"] + [repos()["repo"][repo_name]["test"]]:
        p = prefix(c)
        if p not in seen:
            seen.append(p)
    return seen


def allowed_tools(repo_name, name):
    cmds = allowed_commands(repo_name, name) + READ_ONLY
    return ",".join(
        ["Read", "Edit", "Write", "Glob", "Grep"] + [f"Bash({c}*)" for c in cmds]
    )


def code_list(cmds):
    return ", ".join(f"`{c}`" for c in cmds)


def instructions(repo_name, name):
    """The instruction block: one template, only the tool names differ between arms."""
    text = (HERE / "instructions.md").read_text()
    body, hook_line = text.split("<!-- hooks", 1)
    hook_line = hook_line.split("\n", 1)[1].strip()
    f = family(repo_name, name)
    vals = {
        "check": code_list(f["check"]),
        "fix": code_list(f["fix"]),
        "hook_fix": code_list(f.get("hook_stop", f["hook_fix"])),
        "test": code_list([repos()["repo"][repo_name]["test"]]),
        "allowed": code_list(allowed_commands(repo_name, name) + READ_ONLY),
        "hook_lint": f["hook_lint"]
        .replace(" {file}", " FILE")
        .replace(" {package}", " PACKAGE"),
    }
    vals["hooks"] = ("\n" + hook_line.format(**vals)) if arm(name).get("hooks") else ""
    return re.sub(r"\n{3,}", "\n\n", body.format(**vals)).rstrip() + "\n"


def settings(name, meta):
    """The --settings JSON of a run: the hooks of the hook arms (Sudsy's `sudsy hook`, hook.py for the
    native tools), nothing otherwise."""
    a = arm(name)
    if not a.get("hooks"):
        return {}
    if a["family"] == "sudsy":
        # Sudsy's own hooks, the commands `sudsy init --agents` registers
        post, stop = f"{meta}/bin/sudsy hook post-edit", f"{meta}/bin/sudsy hook stop"
        return {
            "hooks": {
                "PostToolUse": [
                    {
                        "matcher": "Edit|Write|MultiEdit",
                        "hooks": [{"type": "command", "command": post}],
                    }
                ],
                "Stop": [{"hooks": [{"type": "command", "command": stop}]}],
            }
        }
    hook = f"python3 -I {HERE / 'hook.py'}"
    return {
        "hooks": {
            "PostToolUse": [
                {
                    "matcher": "Edit|Write|MultiEdit",
                    "hooks": [{"type": "command", "command": f"{hook} post {meta}"}],
                }
            ],
            "Stop": [
                {"hooks": [{"type": "command", "command": f"{hook} stop {meta}"}]}
            ],
        }
    }


if __name__ == "__main__":
    import sys

    repo, name = sys.argv[1], sys.argv[2]
    print(instructions(repo, name))
    print(allowed_tools(repo, name))
    print(json.dumps(settings(name, "META")))
