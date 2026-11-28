"""matched.py --sudsy PATH [REPO...]: writes configs/<repo>.sudsy.yaml for the stack comparison (pair B):
the Sudsy rules that map to the rules the repository's own lint config selects (resolved by the tool
itself at the pin: `ruff check --show-settings`, `eslint --print-config`, oxlint's default set), every
other Sudsy rule of the language off, and only the files the native tools check. Each file's header
records the mapping: the native rules with no Sudsy rule stay on the native side only."""

import argparse
import json
import pathlib
import re
import subprocess
import tempfile

import yaml

from common import CACHE, HERE, REPO_ROOT, repos, run_env
from fetch import export

PARITY = REPO_ROOT / "docs" / "ai" / "parity"
OUT = HERE / "configs"


def catalog(name):
    """{native rule: sudsy rule} from a parity catalog's `rule <id>` entries."""
    entries = yaml.safe_load((PARITY / f"{name}.yaml").read_text())["entries"]
    out = {}
    for k, v in entries.items():
        m = re.match(r"rule ([a-z0-9-]+)", str(v))
        if m:
            out[str(k)] = m.group(1)
    return out


def sudsy_rules(sudsy, language):
    r = subprocess.run([sudsy, "rules"], capture_output=True, text=True, check=True)
    return sorted(
        {
            l.split()[0]
            for l in r.stdout.splitlines()
            if len(l.split()) > 1 and l.split()[1] == language
        }
    )


def ruff_selected(tree, env, probe):
    r = subprocess.run(
        ["ruff", "check", "--show-settings", probe],
        cwd=tree,
        env=env,
        capture_output=True,
        text=True,
        check=True,
    )
    block = r.stdout.split("linter.rules.enabled = [", 1)[1].split("]", 1)[0]
    return sorted(set(re.findall(r"\(([A-Z]+[0-9]+)\)", block)))


def eslint_selected(tree, env, probe):
    r = subprocess.run(
        ["node_modules/.bin/eslint", "--print-config", probe],
        cwd=tree,
        env=env,
        capture_output=True,
        text=True,
        check=True,
    )
    out = {}
    for name, v in json.loads(r.stdout)["rules"].items():
        sev = v[0] if isinstance(v, list) else v
        if sev not in (0, "off"):
            out[name] = "error" if sev in (2, "error") else "warn"
    return out


def write(
    repo,
    language,
    include,
    exclude,
    rules,
    mapped_from,
    unmapped,
    sudsy,
    overrides=None,
):
    lines = [
        f"# The stack comparison's Sudsy config for {repo} (tools/agent-bench/matched.py; do not edit by hand).",
        f"# Native rules mapped to a Sudsy rule ({len(mapped_from)}): "
        + ", ".join(f"{k}->{v}" for k, v in sorted(mapped_from.items())),
        f"# Native rules with no Sudsy rule, checked on the native side only ({len(unmapped)}): "
        + ", ".join(sorted(unmapped)),
        "# Every other Sudsy rule of the language is off; only the files the native tools check are included.",
    ]
    cfg = {"include": include}
    if exclude:
        cfg["exclude"] = exclude
    cfg[language] = {
        "lint": {
            "rules": {r: rules.get(r, "off") for r in sudsy_rules(sudsy, language)}
        }
    }
    if overrides:
        cfg["overrides"] = overrides
    OUT.mkdir(exist_ok=True)
    with open(OUT / f"{repo}.sudsy.yaml", "w", newline="\n") as f:
        f.write("\n".join(lines) + "\n" + yaml.safe_dump(cfg, sort_keys=False))
    print(f"{repo}: {len(mapped_from)} mapped, {len(unmapped)} unmapped")


def python_repo(repo, sudsy, tree, env, probe, include, exclude, test_ignores=()):
    codes = ruff_selected(tree, env, probe)
    cat = catalog("ruff")
    mapped = {c: cat[c] for c in codes if c in cat}
    rules = {v: "warn" for v in mapped.values()}
    over = None
    off = sorted(
        {cat[c] for c in codes if c in cat and c.startswith(tuple(test_ignores))}
    )
    if off:
        over = [
            {
                "files": ["tests/**"],
                "python": {"lint": {"rules": {r: "off" for r in off}}},
            }
        ]
    write(
        repo,
        "python",
        include,
        exclude,
        rules,
        mapped,
        [c for c in codes if c not in cat],
        sudsy,
        over,
    )


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--sudsy", required=True)
    ap.add_argument("repos", nargs="*", default=["click", "humanize", "ufo", "pathe"])
    a = ap.parse_args()
    for repo in a.repos:
        cfg = {**repos()["repo"][repo], "name": repo}
        env = run_env(cfg)
        with tempfile.TemporaryDirectory() as tmp:
            tree = pathlib.Path(tmp)
            export(repo, cfg["pin"], tree)
            if "node" in cfg:
                (tree / "node_modules").symlink_to(
                    CACHE / "node" / repo / "node_modules"
                )
            if repo == "click":
                python_repo(
                    repo,
                    a.sudsy,
                    tree,
                    env,
                    "src/click/core.py",
                    ["**/*.py"],
                    ["examples/**"],
                )
            elif repo == "humanize":
                python_repo(
                    repo,
                    a.sudsy,
                    tree,
                    env,
                    "src/humanize/time.py",
                    ["**/*.py"],
                    [],
                    test_ignores=("D", "DTZ"),
                )
            elif repo == "ufo":
                sel = eslint_selected(tree, env, "src/utils.ts")
                cat = {
                    **catalog("eslint"),
                    **{
                        f"@typescript-eslint/{k}": v
                        for k, v in catalog("typescript-eslint").items()
                    },
                }
                mapped = {k: cat[k] for k in sel if k in cat}
                rules = {cat[k]: sel[k] for k in mapped}
                write(
                    repo,
                    "js",
                    ["**/*.ts", "**/*.mjs", "**/*.js"],
                    ["dist/**"],
                    rules,
                    mapped,
                    [k for k in sel if k not in cat],
                    a.sudsy,
                )
            elif repo == "pathe":
                # .oxlintrc.json selects oxlint's default set (correctness) with its default plugins: the preset
                preset = yaml.safe_load(
                    (
                        REPO_ROOT / "crates" / "lang-js" / "presets" / "oxlint.yaml"
                    ).read_text()
                )
                head = (
                    REPO_ROOT / "crates" / "lang-js" / "presets" / "oxlint.yaml"
                ).read_text().split("\n")[2]
                unmapped = (
                    head.split(": ", 1)[1].split(", ")
                    if "no mapped Sudsy rule" in head
                    else []
                )
                rules = preset["js"]["lint"]["rules"]
                write(
                    repo,
                    "js",
                    ["**/*.ts", "**/*.mjs", "**/*.js"],
                    ["dist/**"],
                    rules,
                    {r: r for r in rules},
                    unmapped,
                    a.sudsy,
                )


if __name__ == "__main__":
    main()
