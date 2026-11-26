"""Lint-level evidence of the Rust lint leg (docs/ai/design/rust.md): the clippy lints a corpus file sets with an attribute
(`#![warn(clippy::pedantic)]`, `#[allow(clippy::too_many_lines)]`, ...) and the levels a Cargo.toml sets in `[lints.clippy]`
or `[workspace.lints.clippy]`, and the keys of the clippy.toml files. Counted per level, per lint, with files and projects.

Usage: python3 -I scripts/evidence_rust_lints.py
Not here: the clippy findings themselves. A clippy run needs each project's dependencies and types (cargo), and the corpus
is single files for the most part; that checklist is not run (docs/ai/design/rust.md, Not verified).
"""
import re
import sys
from collections import Counter, defaultdict
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import evidence
import evidence_rust as mod

ATTR = re.compile(r"#!?\[\s*(allow|warn|deny|forbid|expect|expect)\s*\(([^\]]*)\)\s*\]")
LINT = re.compile(r"clippy::([a-z_]+)")
LINTS_TABLE = re.compile(r"^\[(workspace\.)?lints\.clippy\]\s*$")
LEVEL_KEY = re.compile(r'^\s*([a-z_]+)\s*=\s*(?:"(allow|warn|deny|forbid)"|\{\s*level\s*=\s*"(allow|warn|deny|forbid)")')
CLIPPY_TOML_KEY = re.compile(r"^\s*([a-z_]+)\s*=", re.M)


def files_of(folder_name):
    for p, n, f in evidence.files(mod):
        if f.is_file():
            yield p, f


def manifests():
    """Cargo.toml and clippy.toml files of the corpus (the evidence module lists only .rs files), test data left out."""
    root = evidence.CORPUS / "rust"
    for f in sorted(root.rglob("*")):
        if f.is_file() and f.name in ("Cargo.toml", "clippy.toml", ".clippy.toml") and not mod.SKIP.search(str(f)):
            yield f.relative_to(root).parts[0], f


def main():
    corpus = list(files_of("rust"))
    extra = list(manifests())
    # attribute lints: level -> lint -> [occurrences, files, projects]
    attr = defaultdict(lambda: defaultdict(lambda: [0, set(), set()]))
    table_levels = defaultdict(lambda: defaultdict(lambda: [0, set(), set()]))
    clippy_toml = Counter()
    seen_projects = set()
    for proj, f in corpus + extra:
        text = evidence.read(f) or ""
        if "clippy::" in text:
            for m in ATTR.finditer(text):
                level, body = m.group(1), m.group(2)
                for lint in LINT.findall(body):
                    r = attr[level][lint]
                    r[0] += 1
                    r[1].add(str(f))
                    r[2].add(proj)
                for group in re.findall(r"clippy::(all|pedantic|nursery|cargo|restriction)\b", body):
                    r = attr[level]["group:" + group]
                    r[0] += 1
                    r[1].add(str(f))
                    r[2].add(proj)
        if f.name == "Cargo.toml":
            inside = False
            for line in text.splitlines():
                if line.strip().startswith("["):
                    inside = bool(LINTS_TABLE.match(line.strip()))
                    continue
                if inside:
                    m = LEVEL_KEY.match(line)
                    if m:
                        lint = m.group(1)
                        level = m.group(2) or m.group(3)
                        r = table_levels[level][lint]
                        r[0] += 1
                        r[1].add(str(f))
                        r[2].add(proj)
        if f.name in ("clippy.toml", ".clippy.toml"):
            seen_projects.add(proj)
            for k in CLIPPY_TOML_KEY.findall(text):
                clippy_toml[k] += 1
    print(f"{len(corpus)} files, {len({p for p, _ in corpus})} projects (corpus after the test-data cut)\n")
    print("Attribute lints (`#[level(clippy::NAME)]` in a .rs file): level, lint, occurrences, files, projects")
    rows = sorted(((lv, lint, *r) for lv, d in attr.items() for lint, r in d.items()), key=lambda x: -x[2])
    print("| level | lint | occurrences | files | projects |\n|---|---|---|---|---|")
    for lv, lint, n, fs, ps in rows:
        print(f"| {lv} | {lint} | {n} | {len(fs)} | {len(ps)} |")
    print("\n[lints.clippy] tables in Cargo.toml: level, lint, occurrences, files, projects")
    print("| level | lint | occurrences | files | projects |\n|---|---|---|---|---|")
    for lv, d in sorted(table_levels.items()):
        for lint, (n, fs, ps) in sorted(d.items(), key=lambda x: -x[1][0]):
            print(f"| {lv} | {lint} | {n} | {len(fs)} | {len(ps)} |")
    print(f"\nclippy.toml files: {len(seen_projects)}; keys: {dict(clippy_toml)}")


if __name__ == "__main__":
    main()
