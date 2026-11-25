"""Evidence run of a format leg with no reference tool (docs/ai/development/no-reference-tool.md): split, decision table, agreement, comparison with a partial tool.

Usage: evidence.py <language> <sudsy-binary> <work-dir> [<tool-binary> [<cause> [<n>]]]
       evidence.py <language> list   (project, half and path of every corpus file, tab-separated)
       evidence.py <language> table  (the decision table alone, for a language with no formatter yet)

`<language>` names a module `scripts/evidence_<language>.py`, which holds the facts of the language and no mechanism:
  DIRS      [(corpus folder below tests/conformance/corpus, regex of file names)]
  PROJECT   {repository folder: project} for repositories that are one project; the folder name otherwise
  SKIP      optional: regex of corpus paths that are not the language (left out)
  CAP       optional: at most this many files per project (the first by path), so that no project outweighs the others
  SEED      seed of the split (random.Random over the sorted project names; the first half decides, the other is held out)
  KLASSES   {class: text} of causes of difference that a comparison may leave out, in the order they are left out (cumulative)
  TOOL      argv after the binary that reads a file on stdin and writes the result on stdout
  SUDSY     {row: variant Sudsy writes (a string or a set of strings), or None when the row is kept as written}
  variants(text)   {row: [variant of every occurrence]} of one file, independent of the formatter
  causes(a, b)     [(cause, class)] of the differences of two outputs (a: Sudsy, b: the tool); class is a key of KLASSES or ""
The corpus is only read; the tools get copies in <work-dir>. Prints the corpus, the split, the decision table with the
agreement on the held-out half, the comparison with the tool (equal files, the 15 largest causes), a sample of 30 changed
files and the check of the rows against what the formatter changes.
"""
import importlib
import random
import subprocess
import sys
from collections import Counter, defaultdict
from concurrent.futures import ThreadPoolExecutor
from difflib import unified_diff
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CORPUS = ROOT / "tests/conformance/corpus"
sys.path.insert(0, str(Path(__file__).resolve().parent))


def files(mod):
    """[(project, name, path)] of the corpus folders of the language."""
    out, taken = [], Counter()
    for d, names in mod.DIRS:
        for f in sorted((CORPUS / d).rglob("*")):
            if f.is_file() and names.match(f.name) and not (hasattr(mod, "SKIP") and mod.SKIP.search(str(f))):
                rel = f.relative_to(CORPUS / d)
                project = mod.PROJECT.get(rel.parts[0], rel.parts[0])
                taken[project] += 1
                if taken[project] <= getattr(mod, "CAP", 1 << 30):
                    out.append((project, f"{d}/{rel}", f))
    return out


def read(f, keep_eol=False):
    """The text of a file; `keep_eol`: carriage returns stay (a module whose rows count line endings sets KEEP_EOL)."""
    try:
        return f.read_bytes().decode("utf-8") if keep_eol else f.read_text(encoding="utf-8")
    except UnicodeDecodeError:
        return None


def forms(mod, row):
    s = mod.SUDSY[row]
    return None if s is None else ({s} if isinstance(s, str) else set(s))


def split(corpus, seed):
    names = sorted({p for p, _, _ in corpus})
    random.Random(seed).shuffle(names)
    half = len(names) // 2 + len(names) % 2
    return set(names[:half]), set(names[half:])


def count(corpus, deciding, mod):
    """per[half][row][project] = Counter of variants."""
    per = {"deciding": defaultdict(lambda: defaultdict(Counter)), "held": defaultdict(lambda: defaultdict(Counter))}
    for proj, _, f in corpus:
        text = read(f, getattr(mod, "KEEP_EOL", False))
        if text is None:
            continue
        for row, vs in mod.variants(text).items():
            per["deciding" if proj in deciding else "held"][row][proj].update(vs)
    return per


def report_table(mod, deciding, held, per):
    print(f"seed {mod.SEED}; deciding ({len(deciding)}): {', '.join(sorted(deciding))}")
    print(f"held out ({len(held)}): {', '.join(sorted(held))}\n")
    print("| row | projects | votes (deciding) | rule | Sudsy writes | agrees | held-out occurrences | held-out with Sudsy form |")
    print("|---|---|---|---|---|---|---|---|")
    tot = [0, 0]
    macro = []
    for row in mod.SUDSY:
        votes = Counter(c.most_common(1)[0][0] for c in per["deciding"][row].values() if c)
        n = sum(votes.values())
        top, topn = votes.most_common(1)[0] if votes else ("-", 0)
        share = topn / n if n else 0
        rule = "default (>=80%)" if share >= 0.8 else "default + option (60-80%)" if share >= 0.6 else "as written (<60%)"
        occ = Counter()
        for c in per["held"][row].values():
            occ.update(c)
        total = sum(occ.values())
        want = forms(mod, row)
        mine = sum(occ[v] for v in want) if want else 0
        if want:
            tot[0] += mine
            tot[1] += total
            shares = [sum(c[v] for v in want) / sum(c.values()) for c in per["held"][row].values() if c]
            if shares:
                macro.append(sum(shares) / len(shares))
        vs = ", ".join(f"{v} {k}" for k, v in votes.most_common(4))
        agrees = "-" if not want else "yes" if top in want and share >= 0.6 else "no"
        print(f"| {row} | {n} | {vs} | {rule} | {'/'.join(sorted(want)) if want else 'as written'} | {agrees} | {total} | {mine} ({100 * mine / max(total, 1):.1f}%) |")
    print(f"\nagreement (held out, pooled over the decided rows): {tot[0]}/{tot[1]} = {100 * tot[0] / max(tot[1], 1):.1f}%; mean of the rows' project means {100 * sum(macro) / max(len(macro), 1):.1f}%")
    print("\nvariant counts per project, deciding half:")
    for row in mod.SUDSY:
        print(f"- {row}:")
        for p, c in sorted(per["deciding"][row].items()):
            print(f"    {p}: {dict(c)}")
    print("\nvariant counts, held-out half (occurrences):")
    for row in mod.SUDSY:
        occ = Counter()
        for c in per["held"][row].values():
            occ.update(c)
        print(f"- {row}: {dict(occ)}")


def run_sudsy(sudsy, work, corpus):
    def one(arg):
        i, (_, _, f) = arg
        d = work / "sudsy" / str(i)
        d.mkdir(parents=True, exist_ok=True)
        (d / f.name).write_bytes(f.read_bytes())
        r = subprocess.run([sudsy, "format", f.name], cwd=d, capture_output=True, text=True, encoding="utf-8", check=False)
        return None if r.returncode else (d / f.name).read_text(encoding="utf-8")
    with ThreadPoolExecutor(8) as ex:
        return list(ex.map(one, enumerate(corpus)))


def run_tool(tool, args, work, corpus):
    """The tool on stdin, from an empty folder with its own home (it must not read a configuration)."""
    (work / "tool").mkdir(parents=True, exist_ok=True)
    env = {"PATH": "/usr/bin:/bin", "HOME": str(work / "tool"), "XDG_CONFIG_HOME": str(work / "tool")}
    def one(f):
        try:
            r = subprocess.run([tool, *args], cwd=work / "tool", env=env, input=f[2].read_bytes(), capture_output=True, check=False, timeout=120)
        except (OSError, subprocess.TimeoutExpired):
            return None
        return None if r.returncode else r.stdout.decode("utf-8", "replace")
    with ThreadPoolExecutor(8) as ex:
        return list(ex.map(one, corpus))


def compare(mod, rows):
    """rows: [(project, name, sudsy output, tool output)] where both succeeded. Counts per cause class, cumulative."""
    order = list(mod.KLASSES)
    n, equal = len(rows), 0
    left_out = [0] * len(order)
    causes_all, files_by = Counter(), defaultdict(set)
    for _, name, s, d in rows:
        if s == d:
            equal += 1
            continue
        cs = mod.causes(s, d)
        for k, c in enumerate(order):
            if all(cl in order[: k + 1] for _, cl in cs):
                left_out[k] += 1
        for key in cs:
            causes_all[key] += 1
            files_by[key].add(name)
    return n, equal, left_out, causes_all, files_by


def main():
    lang = sys.argv[1]
    mod = importlib.import_module(f"evidence_{lang}")
    corpus = files(mod)
    if sys.argv[2] == "list":
        deciding, _ = split(corpus, mod.SEED)
        for p, _, f in corpus:
            print(f"{p}\t{'deciding' if p in deciding else 'held'}\t{f}")
        return
    if sys.argv[2] == "table":  # no formatter yet: the decision table of the corpus alone
        deciding, held = split(corpus, mod.SEED)
        print(f"{len(corpus)} files, {len({p for p, _, _ in corpus})} projects\n")
        report_table(mod, deciding, held, count(corpus, deciding, mod))
        return
    sudsy, work = sys.argv[2], Path(sys.argv[3])
    tool = sys.argv[4] if len(sys.argv) > 4 else None
    projects = {p for p, _, _ in corpus}
    print(f"{len(corpus)} files, {len(projects)} projects\n")
    deciding, held = split(corpus, mod.SEED)
    per = count(corpus, deciding, mod)
    report_table(mod, deciding, held, per)
    sud = run_sudsy(sudsy, work, corpus)
    texts = [read(f) for _, _, f in corpus]
    print(f"\nSudsy failed on {sum(s is None for s in sud)} files:")
    for (_, n, _), s in zip(corpus, sud):
        if s is None:
            print(f"  failed: {n}")
    check_model(mod, corpus, texts, sud)
    check_meaning(mod, corpus, texts, sud)
    changed = [(c, s, t) for c, s, t in zip(corpus, sud, texts) if s is not None and t is not None and s != t]
    sample = random.Random(mod.SEED + 1).sample(changed, min(30, len(changed)))
    print(f"\nsample of {len(sample)} of {len(changed)} files Sudsy changes (diff, first 10 lines each):")
    for (_, n, _), s, t in sample:
        diff = list(unified_diff(t.splitlines(), s.splitlines(), "source", "sudsy", lineterm="", n=0))
        print(f"=== {n}: {len(diff)} diff lines")
        print("\n".join(l[:150] for l in diff[2:12]))
    if tool is None:
        return
    out = run_tool(tool, mod.TOOL, work, corpus)
    print(f"\nthe tool failed on {sum(d is None for d in out)} files")
    ok = [(c, s, d, t) for c, s, d, t in zip(corpus, sud, out, texts) if s is not None and d is not None and t is not None]
    for label, who in (("held out", lambda p: p in held), ("deciding", lambda p: p in deciding), ("all", lambda p: True)):
        sub = [(c[0], c[1], s, d) for c, s, d, t in ok if who(c[0])]
        su = sum(s == t for c, s, d, t in ok if who(c[0]))
        du = sum(d == t for c, s, d, t in ok if who(c[0]))
        n, eq, lo, ca, fb = compare(mod, sub)
        line = f"\n[{label}] both succeed: {n}; equal: {eq} ({100 * eq / max(n, 1):.1f}%); left unchanged by Sudsy {su}, by the tool {du}"
        acc = eq
        for k, c in enumerate(mod.KLASSES):
            line += f"; equal if {mod.KLASSES[c]} left out: {eq + lo[k]} ({100 * (eq + lo[k]) / max(n, 1):.1f}%)"
        print(line)
        if label == "all":
            for (c, cl), k in ca.most_common(15):
                print(f"  {k:6d} {len(fb[(c, cl)]):4d} files  {'[' + cl + '] ' if cl else ''}{c}")
            if len(sys.argv) > 5 and hasattr(mod, "EXAMPLES"):
                for e in mod.EXAMPLES.get(sys.argv[5], [])[: int(sys.argv[6]) if len(sys.argv) > 6 else 3]:
                    print("--- sudsy\n" + e[0] + "\n--- tool\n" + e[1])


def check_meaning(mod, corpus, texts, sud):
    """The module's own oracle on the formatter's output: where it may have changed what the code means."""
    if not hasattr(mod, "meaning"):
        return
    by, files_by = Counter(), defaultdict(list)
    for (_, name, _), s, t in zip(corpus, sud, texts):
        if s is None or t is None or s == t:
            continue
        for line, what in mod.meaning(t, s):
            by[what] += 1
            files_by[what].append(f"{name}:{line}")
    print(f"\nmeaning check of {sum(s is not None and s != t for s, t in zip(sud, texts))} changed files: " + (", ".join(f"{k}: {v} in {len(set(f.split(':')[0] for f in files_by[k]))} files" for k, v in by.items()) or "nothing found"))
    for what, where in files_by.items():
        for w in where[:6]:
            print(f"  {what}: {w}")


def check_model(mod, corpus, texts, sud):
    """The rows against the formatter: a file whose occurrences all have the form Sudsy writes and that Sudsy still changes
    is a layout choice no row covers; a file with an occurrence of another form that Sudsy leaves unchanged is a row the
    formatter does not apply (kept as written, or a defect of the row)."""
    uncovered, unapplied = [], []
    for (_, name, _), s, t in zip(corpus, sud, texts):
        if s is None or t is None:
            continue
        vs = mod.variants(t)
        agree = all(v in forms(mod, row) for row in mod.SUDSY if forms(mod, row) for v in vs.get(row, []))
        if agree and s != t:
            uncovered.append(name)
        if not agree and s == t:
            unapplied.append(name)
    print(f"\nrows against the formatter: {len(uncovered)} files changed with every occurrence in the written form (a choice no row covers), {len(unapplied)} files left unchanged with an occurrence of another form")
    for label, names in (("uncovered", uncovered), ("unapplied", unapplied)):
        for n in random.Random(mod.SEED + 2).sample(names, min(8, len(names))):
            print(f"  {label}: {n}")


if __name__ == "__main__":
    main()
