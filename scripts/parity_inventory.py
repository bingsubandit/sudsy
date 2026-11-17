"""Adds the upstream rules a parity catalog lacks, as `planned`, from the tool's pinned version.

Usage: parity_inventory.py <tool>... | --all  [--write]  (without --write: only prints what is missing)

The source of each tool is SOURCES below: an npm package loaded by Node (`js`: the body of an async
function returning the rule ids, or `{id, deprecated}` objects), or a raw file of the tool's repository at its tag (`raw`, `re`), or a PyPI wheel (`ruff rule --all`,
or the files of the wheel matched by a regex). The version is the catalog's `version`, or the group
of `version_re` in it. Ids missing from `entries:` and the top-level `deferred:` groups are appended
to `entries:`; a deprecated upstream rule is added as `wontfix deprecated upstream`. Entries are
never changed or removed: ids the catalog has and the tool lacks are printed (a renamed or removed
rule). The catalogs of tools with no source here are inventories already (see docs/development/parity.md).
"""
import io
import json
import os
import platform
import re
import subprocess
import sys
import tarfile
import urllib.request
import zipfile
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parent.parent
PARITY = ROOT / os.environ.get("CARGO_TARGET_DIR", "target") / "parity"

PLUGIN = "const m=await import('{pkg}');const p=m.default??m;return Object.entries(p.rules??m.rules).map(([id,r])=>({{id,deprecated:!!r.meta?.deprecated,ext:!!r.meta?.docs?.extensionRule}}));"
SOURCES = {
    "eslint": {"npm": ["eslint"], "js": "const {builtinRules}=await import('eslint/use-at-your-own-risk');return [...builtinRules].map(([id,r])=>({id,deprecated:!!r.meta?.deprecated}));"},
    "typescript-eslint": {"npm": ["@typescript-eslint/eslint-plugin", "eslint", "typescript"], "js": PLUGIN.format(pkg="@typescript-eslint/eslint-plugin")},
    "stylelint": {"npm": ["stylelint"], "js": "const s=(await import('stylelint')).default;return Object.keys(s.rules);"},
    "stylelint-scss": {"npm": ["stylelint-scss", "stylelint"], "js": "const m=await import('stylelint-scss');return (m.default??m).map(p=>p.ruleName);"},
    "stylelint-less": {"npm": ["stylelint-less", "stylelint"], "js": "const m=await import('stylelint-less');return (m.default??m).map(p=>p.ruleName);"},
    "eslint-plugin-vue": {"npm": ["eslint-plugin-vue", "eslint"], "js": PLUGIN.format(pkg="eslint-plugin-vue")},
    "eslint-plugin-svelte": {"npm": ["eslint-plugin-svelte", "eslint", "svelte"], "js": PLUGIN.format(pkg="eslint-plugin-svelte")},
    "eslint-plugin-jsonc": {"npm": ["eslint-plugin-jsonc", "eslint"], "prefix": "jsonc/", "js": PLUGIN.format(pkg="eslint-plugin-jsonc")},
    "angular-eslint": {"npm": ["@angular-eslint/eslint-plugin-template", "eslint", "typescript", "@angular-eslint/template-parser"], "js": PLUGIN.format(pkg="@angular-eslint/eslint-plugin-template")},
    "graphql-eslint": {"npm": ["@graphql-eslint/eslint-plugin", "graphql", "eslint"], "js": PLUGIN.format(pkg="@graphql-eslint/eslint-plugin")},
    "html-eslint": {"npm": ["@html-eslint/eslint-plugin", "eslint"], "js": PLUGIN.format(pkg="@html-eslint/eslint-plugin")},
    "html-validate": {"npm": ["html-validate"], "js": "const m=await import('html-validate');return Object.keys(m.builtinRules);"},
    "htmlhint": {"npm": ["htmlhint"], "js": "const m=await import('htmlhint');const h=m.HTMLHint??m.default.HTMLHint??m.default;return Object.keys(h.rules);"},
    "markdownlint": {"npm": ["markdownlint"], "version_re": r"markdownlint (\d[\w.]*)\)", "js": "const m=await import(process.cwd()+'/node_modules/markdownlint/lib/rules.mjs');return m.default.map(r=>r.names[0]);"},
    "roslynator": {"raw": "https://raw.githubusercontent.com/dotnet/roslynator/v{version}/src/Analyzers.xml", "re": r"<Id>(RCS\d+)</Id>"},
    "tflint": {"raw": "https://raw.githubusercontent.com/terraform-linters/tflint-ruleset-terraform/v{version}/docs/rules/README.md", "re": r"\b(terraform_[a-z_]+)\b"},
    "hadolint": {"raw": "https://raw.githubusercontent.com/hadolint/hadolint/v{version}/README.md", "re": r"\b(DL\d{4})\b"},
    "rubocop": {"raw": "https://raw.githubusercontent.com/rubocop/rubocop/v{version}/config/default.yml", "re": r"(?m)^(Lint/\w+):"},
    "ruff": {"wheel": "ruff", "ruff": True},
    "yamllint": {"wheel": "yamllint", "files": r"^yamllint/rules/([a-z][a-z_]*)\.py$"},
    "sqlfluff": {"wheel": "sqlfluff", "files": r"^sqlfluff/rules/.*\.py$", "content": r"class Rule_([A-Z]{2}\d{2})\b"},
}


def catalog_path(tool):
    return ROOT / "docs/parity" / f"{tool}.yaml"


def pinned(tool):
    text = catalog_path(tool).read_text(encoding="utf-8")
    version = re.search(r'^version: "(.*)"', text, re.M).group(1)
    pattern = SOURCES[tool].get("version_re")
    return re.search(pattern, version).group(1) if pattern else version.split()[0]


def wheel(name, version):
    """The wheel of `name` for this machine (PyPI's JSON API, no pip), as a ZipFile."""
    files = json.load(urllib.request.urlopen(f"https://pypi.org/pypi/{name}/{version}/json"))["urls"]
    machine = platform.machine().lower()
    for f in files:
        n = f["filename"]
        if n.endswith(".whl") and ("none-any" in n or ("manylinux" in n and machine in n)):
            return zipfile.ZipFile(io.BytesIO(urllib.request.urlopen(f["url"]).read()))
    sys.exit(f"{name} {version}: no wheel for {machine}")


def upstream(tool):
    """`{id: catalog status}` of the pinned version's rules."""
    src, version = SOURCES[tool], pinned(tool)
    if "npm" in src:
        prefix = PARITY / f"inventory-{tool}-{version}"
        if not (prefix / "node_modules" / src["npm"][0]).is_dir():
            prefix.mkdir(parents=True, exist_ok=True)
            specs = [f"{src['npm'][0]}@{version}", *src["npm"][1:]]
            subprocess.run(["npm", "install", "--silent", "--no-audit", "--no-fund", "--prefix", str(prefix), *specs], check=True)
        out = subprocess.run(["node", "--input-type=module", "-e", f"console.log(JSON.stringify(await (async()=>{{{src['js']}}})()))"],
                             cwd=prefix, capture_output=True, text=True)
        if out.returncode:
            sys.exit(f"{tool}: node failed:\n{out.stderr}")
        found = {(r["id"] if isinstance(r, dict) else r): "wontfix deprecated upstream" if isinstance(r, dict) and r["deprecated"] else "deferred | extension of an ESLint core rule" if isinstance(r, dict) and r.get("ext") else "planned" for r in json.loads(out.stdout)}
    elif "raw" in src:
        text = urllib.request.urlopen(src["raw"].format(version=version)).read().decode("utf-8", "replace")
        found = {i: "planned" for i in re.findall(src["re"], text)}
    elif src.get("ruff"):
        z = wheel("ruff", version)
        binary = PARITY / f"inventory-ruff-{version}" / "ruff"
        if not binary.exists():
            binary.parent.mkdir(parents=True, exist_ok=True)
            member = next(n for n in z.namelist() if n.endswith("/scripts/ruff"))
            binary.write_bytes(z.read(member))
            binary.chmod(0o755)
        rules = json.loads(subprocess.run([str(binary), "rule", "--all", "--output-format", "json"], capture_output=True, text=True, check=True).stdout)
        status = lambda r: next(iter(r["status"])) if isinstance(r["status"], dict) else r["status"]
        found = {r["code"]: {"Deprecated": "wontfix deprecated upstream", "Removed": "wontfix removed upstream", "Preview": "deferred | preview rule"}.get(status(r), "planned") for r in rules if r["code"]}
    else:
        z = wheel(src["wheel"], version)
        found = {}
        for n in z.namelist():
            m = re.search(src["files"], n)
            if not m:
                continue
            if "content" in src:
                found.update({i: "planned" for i in re.findall(src["content"], z.read(n).decode("utf-8", "replace"))})
            elif m.group(1) != "__init__":
                found[m.group(1).replace("_", "-")] = "planned"
    return {SOURCES[tool].get("prefix", "") + i: d for i, d in found.items() if not i.startswith("@")}  # a plugin that wraps another plugin's rules lists them by `@scope/` name


def catalog_ids(tool):
    """Ids of the catalog without option suffixes (`anchors.forbid-unused-anchors` is `anchors`)."""
    data = yaml.safe_load(catalog_path(tool).read_text(encoding="utf-8"))
    ids = set(data["entries"]) | {i for g in data.get("deferred", []) for i in g["ids"]}
    return ids | {i.split(".")[0] for i in ids}


def inventory(tool, write):
    found, have = upstream(tool), catalog_ids(tool)
    missing = sorted(set(found) - have)
    unknown = sorted(i for i in have - set(found) if "." not in i and i not in ("*",))
    print(f"{tool} {pinned(tool)}: {len(found)} upstream rules, {len(have)} catalog ids, {len(missing)} missing" + (f"; catalog ids the tool lacks: {' '.join(unknown[:12])}{' ...' if len(unknown) > 12 else ''}" if unknown else ""))
    if write and missing:
        path = catalog_path(tool)
        text = path.read_text(encoding="utf-8")
        add = "".join(f"  {i}: {found[i]}\n" for i in missing)
        m = re.search(r"^(?=\S)(?!entries:)", text[text.index("\nentries:") + 1 :], re.M)  # the next top-level key, if any
        end = text.index("\nentries:") + 1 + (m.start() if m else len(text) - text.index("\nentries:") - 1)
        text = text[:end].rstrip("\n") + "\n" + add + text[end:]
        path.write_text(text, encoding="utf-8", newline="\n")
    return len(missing)


if __name__ == "__main__":
    args = [a for a in sys.argv[1:] if a != "--write"]
    for t in SOURCES if args == ["--all"] else args:
        inventory(t, "--write" in sys.argv)
