"""Tool evidence of the Rust format leg (docs/ai/design/rust.md): rustfmt on every corpus file, with the edition and the
configuration of its project, and the rustfmt configuration files of the corpus.

Usage: python3 -I scripts/evidence_rust_tools.py <work-dir>   (the work dir is emptied and filled; rustfmt runs from it)

Per file: the edition is the `edition` key of the nearest Cargo.toml above the file inside its repository folder (2021 when
none: a batch 2 file is a single blob, see tests/conformance/fetch-rust-corpus.sh); the configuration is the nearest
rustfmt.toml or .rustfmt.toml above it (none: rustfmt defaults). rustfmt reads the file on stdin, so the out-of-line modules
of a file are not formatted with it (each file is its own run). Outcome: unchanged (output equals input), changed, or
parse error (rustfmt exits non-zero: the file is not Rust for this edition).
"""
import os
import re
import subprocess
import sys
from collections import Counter, defaultdict
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import evidence
import evidence_rust as mod

TOOLCHAIN = "1.99.0"  # rust-toolchain.toml of this repository
EDITION = re.compile(r'^edition\s*=\s*"(\d{4})"', re.M)
OPTION = re.compile(r"^\s*([a-z_]+)\s*=", re.M)
CONFIG_NAMES = ("rustfmt.toml", ".rustfmt.toml")


def repo_root(f):
    """The corpus folder of a file (the repository), the top of its search for Cargo.toml and rustfmt.toml."""
    return evidence.CORPUS / "rust" / f.relative_to(evidence.CORPUS / "rust").parts[0]


def above(f, names):
    """The nearest file named in `names` from f's folder up to its repository folder, or None."""
    top = repo_root(f)
    d = f.parent
    while True:
        for n in names:
            if (d / n).is_file():
                return d / n
        if d == top:
            return None
        d = d.parent


def edition_of(f):
    c = above(f, ("Cargo.toml",))
    if c:
        m = EDITION.search(c.read_text(encoding="utf-8", errors="replace"))
        if m:
            return m.group(1)
    return "2021"


def run(rustfmt, work, edition, config, text):
    args = [rustfmt, "--edition", edition]
    if config:
        args += ["--config-path", str(config)]
    # the work dir has no rust-toolchain file: the toolchain is the repository's pin (rust-toolchain.toml)
    env = dict(os.environ, RUSTUP_TOOLCHAIN=TOOLCHAIN)
    r = subprocess.run(args, cwd=work, env=env, input=text.encode("utf-8"), capture_output=True, check=False, timeout=120)
    if r.returncode:
        err = r.stderr.decode("utf-8", "replace").strip().splitlines()
        return "parse error", (err[0] if err else "")
    out = r.stdout.decode("utf-8", "replace")
    return ("unchanged" if out == text else "changed"), out


def main():
    work = Path(sys.argv[1])
    work.mkdir(parents=True, exist_ok=True)
    corpus = [(p, n, f) for p, n, f in evidence.files(mod) if f.is_file()]
    texts = {}
    for _, _, f in corpus:
        t = evidence.read(f, True)
        if t is not None:
            texts[f] = t
    jobs = [(f, edition_of(f), above(f, CONFIG_NAMES)) for _, _, f in corpus if f in texts]
    rustfmt = "rustfmt"
    with ThreadPoolExecutor(8) as ex:
        res = list(ex.map(lambda j: run(rustfmt, work, j[1], j[2], texts[j[0]]), jobs))
    tally = Counter()
    by_ed = defaultdict(Counter)
    for (f, ed, cfg), (outcome, _) in zip(jobs, res):
        tally[outcome] += 1
        by_ed[ed][outcome] += 1
    n = len(jobs)
    # the list the parser measurement reads (scripts/evidence_rust_parsers): path and edition of every file read
    with open(work / "list.tsv", "w", newline="\n", encoding="utf-8") as fh:
        for f, ed, _ in jobs:
            fh.write(f"{f}\t{ed}\n")
    print(f"files read: {n}; rustfmt (defaults, or the file's own rustfmt.toml):")
    for k in ("unchanged", "changed", "parse error"):
        print(f"  {k}: {tally[k]} ({100 * tally[k] / max(n, 1):.1f}%)")
    print("by edition (from the nearest Cargo.toml; 2021 when none):")
    for ed in sorted(by_ed):
        c = by_ed[ed]
        print(f"  {ed}: " + ", ".join(f"{k} {c[k]}" for k in ("unchanged", "changed", "parse error")))
    # the files that fail: does edition 2024 parse them (a file of a newer crate than its Cargo.toml of 2021 shows, or a
    # single file whose edition is the assumed 2021)?
    fails = [(f, ed, cfg) for (f, ed, cfg), (o, _) in zip(jobs, res) if o == "parse error"]
    with ThreadPoolExecutor(8) as ex:
        again = list(ex.map(lambda j: run(rustfmt, work, "2024", j[2], texts[j[0]])[0], fails))
    print(f"parse errors re-run as edition 2024: {Counter(again)}")
    print("parse errors (file, rustfmt's first message):")
    for (f, ed, _), (outcome, msg) in zip(jobs, res):
        if outcome == "parse error":
            print(f"  {ed} {f.relative_to(evidence.CORPUS)}: {msg}")
    cfgs = sorted({c for _, _, c in jobs if c})
    print(f"\nrustfmt configuration files in the corpus: {len(cfgs)}")
    opts = Counter()
    for c in cfgs:
        opts.update(set(OPTION.findall(c.read_text(encoding="utf-8", errors="replace"))))
        print(f"  {c.relative_to(evidence.CORPUS)}: {c.read_text(encoding='utf-8', errors='replace').strip().replace(chr(10), '; ')[:300]}")
    print("options set (files):", dict(opts))
    manifests = {c for c in (above(f, ("Cargo.toml",)) for _, _, f in corpus) if c}
    eds = Counter(m.group(1) for m in (EDITION.search(c.read_text(encoding="utf-8", errors="replace")) for c in manifests) if m)
    print(f"Cargo.toml files above corpus files: {len(manifests)}; editions:", dict(eds))


if __name__ == "__main__":
    main()
