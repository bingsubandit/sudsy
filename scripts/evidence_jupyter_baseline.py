"""Baseline of the Jupyter plan (docs/ai/design/jupyter.md): ruff on the notebook against Sudsy on the extracted cells. A measuring script, not product code.

Usage: evidence_jupyter_baseline.py format <sudsy-binary> <ruff-binary> <work-dir>
       evidence_jupyter_baseline.py lint <sudsy-binary> <ruff-binary> <work-dir>
Cells are the code cells ruff reads as Python (`ruff_notebook` `is_valid_python_code_cell`). IPython escape lines are replaced by a placeholder
statement before Sudsy sees them (nbQA and black do the same) and put back afterwards; `ruff format` of the same placeholder text as a `.py` file
separates what is notebook-specific from what is the Python formatter's own gap.
"""
import difflib
import json
import pickle
import re
import shutil
import subprocess
import sys
from collections import Counter, defaultdict
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import evidence  # noqa: E402
import evidence_jupyter as mod  # noqa: E402
from evidence_jupyter_cells import load, source  # noqa: E402

AUTOMAGIC = set("""alias alias_magic autoawait autocall automagic bookmark cd code_wrap colors conda config debug dhist dirs doctest_mode edit env gui history
killbgscripts load load_ext loadpy logoff logon logstart logstate logstop lsmagic macro magic mamba matplotlib micromamba notebook page pastebin pdb pdef pdoc
pfile pinfo pinfo2 pip popd pprint precision prun psearch psource pushd pwd pycat pylab quickref recall rehashx reload_ext rerun reset reset_selective run save
sc set_env sx system tb time timeit unalias unload_ext who who_ls whos xdel xmode""".split())
PYCELL_MAGICS = {"capture", "debug", "ipytest", "prun", "pypy", "python", "python3", "time", "timeit"}
ASSIGN_OPS = ("=", "+=", "-=", "*=", "/=", "//=", "%=", "**=", "&=", "|=", "^=")


def is_magic_cell(src):
    """crates/ruff_notebook/src/cell.rs `is_magic_cell`: a cell ruff leaves out of the module."""
    lines = src.split("\n")
    toks = lines[0].split() if lines else []
    if toks and toks[0] in AUTOMAGIC and not (len(toks) > 1 and toks[1] in ASSIGN_OPS):
        return True
    for l in lines:
        t = l.split()
        if t and len(t[0]) >= 2 and t[0].startswith("%%") and t[0][2:] not in PYCELL_MAGICS:
            return True
    return False


def valid_cell(c):
    vs = c.get("metadata", {}).get("vscode") if isinstance(c.get("metadata"), dict) else None
    ok = c.get("cell_type") == "code" and (not isinstance(vs, dict) or vs.get("languageId") == "python")
    return ok and not is_magic_cell(source(c))


def is_python_nb(nb):
    md = nb.get("metadata") if isinstance(nb.get("metadata"), dict) else {}
    li, ks = md.get("language_info"), md.get("kernelspec")
    if isinstance(li, dict) and "name" in li:
        return li["name"] == "python"
    if isinstance(ks, dict):
        return ks.get("language") == "python"
    return True


ASSIGN_ESCAPE = re.compile(r"^(\s*[\w.\[\], ]+?=\s*)([%!].*)$")
HELP = re.compile(r"^\s*(\?{1,2}\S.*|[\w.\[\]()'\"]+\?{1,2})\s*$")


def mask_ipy(text, fill, keep_lines=False):
    """IPython escape lines replaced by `fill(n)`: (masked text, {n: original})."""
    out, table, lines, i = [], {}, text.split("\n"), 0
    while i < len(lines):
        l = lines[i]
        m = ASSIGN_ESCAPE.match(l)
        stmt = l.lstrip().startswith(("%", "!")) or HELP.match(l)
        if m or stmt:
            j = i
            while lines[j].endswith("\\") and j + 1 < len(lines):
                j += 1
            n = len(table)
            head = m.group(2) if m else l.lstrip()
            table[n] = "\n".join([head] + lines[i + 1 : j + 1])
            out.append((m.group(1) if m else l[: len(l) - len(l.lstrip())]) + fill(n))
            out.extend([""] * (j - i) if keep_lines else [])
            i = j + 1
            continue
        out.append(l)
        i += 1
    return "\n".join(out), table


def unmask(text, table):
    for n, orig in table.items():
        text = text.replace(f"__ipy_{n}__", orig)
    return text


def run(cmd, cwd=None):
    return subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, encoding="utf-8", errors="replace", check=False, env={"PATH": "/usr/bin:/bin", "HOME": "/nonexistent"})


def setup(work, corpus, dirs):
    for d in dirs:
        shutil.rmtree(work / d, ignore_errors=True)
        (work / d).mkdir(parents=True)
    for i, (_, _, f) in enumerate(corpus):
        for d in ("orig", "rfmt"):
            if d in dirs:
                shutil.copyfile(f, work / d / f"{i}.ipynb")


def format_baseline(sudsy, ruff, work):
    work = Path(work)
    corpus = evidence.files(mod)
    deciding, held = evidence.split(corpus, mod.SEED)
    setup(work, corpus, ("orig", "rfmt", "cells", "rpy"))
    r = run([ruff, "format", "--isolated", "--no-cache", str(work / "rfmt")])
    failed = {}
    for line in r.stderr.splitlines():
        m = re.match(r"error: (?:Failed to \w+ )?(?:.*/)?(\d+)\.ipynb[: ]*(.*)", line)
        if m:
            failed[int(m.group(1))] = m.group(2)
    print("ruff format:", (r.stderr.strip() or r.stdout.strip()).splitlines()[-1])
    rows, cls, project_of = [], Counter(), {}
    for i, (proj, name, f) in enumerate(corpus):
        project_of[i] = proj
        nb = load(f)
        if not isinstance(nb, dict) or nb.get("nbformat") != 4:
            cls["not nbformat 4 (ruff: invalid format)"] += 1
        elif not is_python_nb(nb):
            cls["non-Python kernel (ruff skips)"] += 1
        elif i in failed:
            cls["ruff fails: " + re.sub(r"\d+", "N", failed[i])[:50]] += 1
        else:
            out = load(work / "rfmt" / f"{i}.ipynb")
            cls["python, ruff ok"] += 1
            for k, c in enumerate(nb["cells"]):
                if valid_cell(c):
                    masked, table = mask_ipy(source(c), lambda n: f"__ipy_{n}__")
                    for d in ("cells", "rpy"):
                        (work / d / f"{i}_{k}.py").write_text(masked + "\n", encoding="utf-8")
                    rows.append((i, k, source(c), source(out["cells"][k]), table))
    print("files by class:", dict(cls.most_common()))
    (work / "cells" / "sudsy.yaml").write_text("extends: [preset:python/ruff]\n")
    run([ruff, "format", "--isolated", "--no-cache", str(work / "rpy")])
    ss = run([sudsy, "format", "."], cwd=work / "cells")
    sfail = {m.group(1) for m in re.finditer(r"^error: (?:.*/)?([\d_]+)\.py", ss.stderr, re.M)}
    print("sudsy format:", (ss.stderr.strip() or ss.stdout.strip()).splitlines()[-1])
    cells, per_file, bad = Counter(), defaultdict(set), []
    for i, k, src, rout, table in rows:
        tag = f"{i}_{k}"
        if tag in sfail:
            res = "sudsy refuses"
            bad.append((i, k, res, src, rout, "", ""))
        else:
            s_out = unmask((work / "cells" / f"{tag}.py").read_text(encoding="utf-8"), table).rstrip("\n")
            r_py = unmask((work / "rpy" / f"{tag}.py").read_text(encoding="utf-8"), table).rstrip("\n")
            res = "equal" if s_out == rout else "notebook only: ruff differs between the cell and the .py" if r_py != rout else "python gap: sudsy differs from ruff on the .py"
            if res != "equal":
                bad.append((i, k, res, src, rout, s_out, r_py))
        cells[res] += 1
        cells[("magic " if table else "plain ") + res] += 1
        per_file[i].add(res)
    print("cells:", json.dumps(dict(sorted(cells.items())), indent=1))
    ok = [i for i in per_file if per_file[i] <= {"equal"}]
    print(f"python files ruff handles: {len(per_file)}; all cells equal: {len(ok)} ({100 * len(ok) / max(len(per_file), 1):.1f}%)")
    (work / "bad.pkl").write_bytes(pickle.dumps((bad, {i: sorted(v) for i, v in per_file.items()}, project_of)))


def ruff_catalog():
    import yaml
    entries = yaml.safe_load((Path(__file__).resolve().parent.parent / "docs/ai/parity/ruff.yaml").read_text())["entries"]
    mapped = {k: (v.split()[1] if v.startswith("rule ") else v.split()[0].rstrip(":")) for k, v in entries.items() if isinstance(v, str)}
    return mapped, {v.split()[1] for v in entries.values() if isinstance(v, str) and v.startswith("rule ")}


def json_after(out):
    return json.JSONDecoder().raw_decode(out[out.index("{"):])[0]


def module_of(nb, fill_stmt="pass", fill_assign="None"):
    """The module ruff builds from a notebook: the valid code cells, each followed by a newline; and the cell of every line."""
    text, cell_of, start_of, placeholders = [], [], {}, set()
    line = 0
    for k, c in enumerate(nb["cells"]):
        if not valid_cell(c):
            continue
        masked, table = mask_ipy(source(c), lambda n: f"pass  # ipy{n}", keep_lines=True)
        masked = re.sub(r"^(\s*[\w.\[\], ]+?=\s*)pass  # ipy\d+$", lambda m: m.group(1) + "None  # ipy", masked, flags=re.M)
        start_of[k] = line
        n = masked.count("\n") + 1
        cell_of.extend([(k + 1, r + 1) for r in range(n)])
        placeholders.update((k + 1, r + 1) for r, l in enumerate(masked.split("\n")) if "# ipy" in l)
        text.append(masked + "\n")
        line += n
    return "".join(text), cell_of, placeholders


def lint_baseline(sudsy, ruff, work):
    work = Path(work)
    corpus = evidence.files(mod)
    deciding, _ = evidence.split(corpus, mod.SEED)
    setup(work, corpus, ("orig", "lint", "lintpy"))
    mapped, rules = ruff_catalog()
    cell_maps, nbs, project_of, holes = {}, set(), {}, {}
    for i, (proj, name, f) in enumerate(corpus):
        project_of[i] = proj
        nb = load(f)
        if isinstance(nb, dict) and nb.get("nbformat") == 4 and is_python_nb(nb) and isinstance(nb.get("cells"), list):
            text, cell_of, ph = module_of(nb)
            cell_maps[i] = cell_of
            holes[i] = ph
            nbs.add(i)
            (work / "lint" / f"{i}.py").write_text(text, encoding="utf-8")
            (work / "lintpy" / f"{i}.py").write_text(text, encoding="utf-8")
    for d in ("orig",):
        for p in list((work / d).glob("*.ipynb")):
            if int(p.stem) not in nbs:
                p.unlink()
    flags = ["check", "--isolated", "--no-cache", "--output-format", "json", "--exit-zero", "--ignore-noqa", "--target-version", "py314"]
    rn = json.loads(run([ruff, *flags, str(work / "orig")]).stdout or "[]")
    rp = json.loads(run([ruff, *flags, str(work / "lintpy")]).stdout or "[]")
    (work / "lint" / "sudsy.yaml").write_text("extends: [preset:python/ruff]\n")
    ss = json_after(run([sudsy, "lint", "--format", "json", "."], cwd=work / "lint").stdout)["diagnostics"]
    idx = lambda path: int(re.search(r"(\d+)\.(?:ipynb|py)$", path).group(1))
    R_nb = Counter((idx(d["filename"]), d["cell"], d["location"]["row"], d["code"]) for d in rn if d["code"] != "invalid-syntax" and (d["cell"], d["location"]["row"]) not in holes[idx(d["filename"])])
    syn_nb = Counter(idx(d["filename"]) for d in rn if d["code"] == "invalid-syntax")
    R_py = Counter()
    for d in rp:
        if d["code"] != "invalid-syntax" and idx(d["filename"]) in cell_maps:
            cell, row = cell_maps[idx(d["filename"])][d["location"]["row"] - 1]
            if (cell, row) not in holes[idx(d["filename"])]:
                R_py[(idx(d["filename"]), cell, row, d["code"])] += 1
    S = Counter()
    for d in ss:
        i = idx(d["path"])
        cell, row = cell_maps[i][d["range"]["start"]["line"] - 1]
        if (cell, row) not in holes[i]:
            S[(i, cell, row, d["rule"])] += 1
    print(f"notebooks linted: {len(nbs)}; ruff findings (default set, cells of Python notebooks): {sum(R_nb.values())} in {len({k[0] for k in R_nb})} files; syntax errors in {len(syn_nb)} files; Sudsy diagnostics: {sum(S.values())}")
    covered = Counter()
    missing = defaultdict(Counter)
    for key, n in R_nb.items():
        i, cell, row, code = key
        rule = mapped.get(code)
        if rule in rules and S.get((i, cell, row, rule), 0):
            covered["covered"] += n
        else:
            why = "in .py too" if R_py.get(key) else "notebook only"
            missing[(code, rule if rule in rules else f"no rule ({rule})")][why] += n
            covered["missing " + why] += n
    tot = sum(R_nb.values())
    print(f"coverage: covered {covered['covered']} of {tot} = {100 * covered['covered'] / tot:.1f}%; missing and ruff reports it on the .py as well {covered['missing in .py too']}, only in the notebook {covered['missing notebook only']}")
    print("missing by ruff code (in .py too / notebook only):")
    for (code, rule), c in sorted(missing.items(), key=lambda t: -sum(t[1].values()))[:25]:
        print(f"  {code} -> {rule}: {c['in .py too']} / {c['notebook only']}")
    inv = {v: k for k, v in mapped.items() if mapped[k] in rules}
    extra = defaultdict(Counter)
    for key, n in S.items():
        i, cell, row, rule = key
        code = inv.get(rule)
        if code and R_nb.get((i, cell, row, code), 0) == 0:
            extra[rule]["also ruff .py" if R_py.get((i, cell, row, code)) else "sudsy only"] += n
    print("Sudsy findings of mapped rules that ruff does not report on the notebook (also reported by ruff on the .py / by Sudsy only):")
    for rule, c in sorted(extra.items(), key=lambda t: -sum(t[1].values()))[:20]:
        print(f"  {rule} ({inv.get(rule)}): {c['also ruff .py']} / {c['sudsy only']}")
    # per half
    for h, who in (("deciding", lambda p: p in deciding), ("held out", lambda p: p not in deciding)):
        t = sum(n for k, n in R_nb.items() if who(project_of[k[0]]))
        c = sum(n for k, n in R_nb.items() if who(project_of[k[0]]) and mapped.get(k[3]) in rules and S.get((k[0], k[1], k[2], mapped[k[3]]), 0))
        print(f"  {h}: covered {c} of {t} = {100 * c / max(t, 1):.1f}%")
    NB_EXTRA = {"no-useless-expression": "notebook: last expression of a cell is no useless expression (B018, B015)", "unsorted-imports": "notebook: import blocks end at a cell",
                "yield-outside-function": "notebook: top-level await is legal (F704)", "undefined-variable": "notebook: IPython builtins (display, get_ipython) (F821)"}
    diffs = defaultdict(set)  # cause -> files
    counts = Counter()
    for key, n in R_nb.items():
        i, cell, row, code = key
        rule = mapped.get(code)
        if rule in rules and S.get((i, cell, row, rule), 0):
            continue
        if not R_py.get(key):
            c = "notebook: import blocks end at a cell" if code == "I001" else f"notebook: other ({code})"
        elif rule in rules:
            c = "python: a mapped rule misses a finding"
        else:
            c = "python: rule not built (planned or deferred)"
        diffs[c].add(i)
        counts[c] += n
    for key, n in S.items():
        i, cell, row, rule = key
        code = inv.get(rule)
        if code and R_nb.get((i, cell, row, code), 0) == 0:
            if rule == "no-useless-expression":
                c = NB_EXTRA[rule]  # a Sudsy-only report is on a last expression too (a tuple of calls), the same exemption hides it
            elif R_py.get((i, cell, row, code)):
                c = NB_EXTRA.get(rule, "notebook: other (ruff .py reports it, ruff notebook not)")
            else:
                c = "python: extra finding of Sudsy"
            diffs[c].add(i)
            counts[c] += n
    for c, n in counts.most_common():
        print(f"  {n:5d} findings {len(diffs[c]):4d} files  {c}")
    order = [c for c, _ in sorted(diffs.items(), key=lambda t: -len(t[1]))]
    broken = defaultdict(set)
    for c, fs in diffs.items():
        for i in fs:
            broken[i].add(c)
    halves = {"all": nbs, "deciding": {i for i in nbs if project_of[i] in deciding}, "held out": {i for i in nbs if project_of[i] not in deciding}}
    print("cumulative: notebooks with every ruff finding covered and no extra Sudsy finding after the causes are fixed")
    for label, fixed in (("only the notebook causes fixed", {c for c in order if c.startswith("notebook")}), ("only the python causes fixed", {c for c in order if c.startswith("python")})):
        print(f"  {label}: " + "; ".join(f"{h} {100 * sum(1 for i in fs if broken[i] <= fixed) / len(fs):.1f}%" for h, fs in halves.items()))
    for step in range(len(order) + 1):
        fixed = set(order[:step])
        row = [f"{h} {sum(1 for i in fs if broken[i] <= fixed)}/{len(fs)} = {100 * sum(1 for i in fs if broken[i] <= fixed) / len(fs):.1f}%" for h, fs in halves.items()]
        print(f"  {'now' if step == 0 else '+ ' + order[step - 1]}: " + "; ".join(row))
    (work / "lint.pkl").write_bytes(pickle.dumps((R_nb, R_py, S, mapped, rules, project_of)))


def cause(res, src, rout, s_out, r_py):
    """The cause class of a differing cell (ruff's notebook output against Sudsy's)."""
    if res == "sudsy refuses":
        return "sudsy refuses (syntax Sudsy's parser rejects)"
    code = lambda t: re.sub(r"\s*#.*$", "", t.rstrip().split("\n")[-1])
    if "notebook only" in res or (code(rout).endswith(";") and not code(s_out).endswith(";")):
        if code(rout).endswith(";"):
            return "notebook: ; after the last statement kept (counts in the width)"
        if src.lstrip().startswith(('"""', "'''", '"', "'")):
            return "notebook: first string of a cell is no module docstring"
        return "notebook: other"
    a, b = rout.split("\n"), s_out.split("\n")
    if [l for l in a if l.strip()] == [l for l in b if l.strip()]:
        return "python gap: blank lines"
    diff = "\n".join(l for l in difflib.unified_diff(a, b, lineterm="", n=0) if l[:1] in "+-" and l[:3] not in ("+++", "---"))
    if "#" in diff:
        return "python gap: trailing comments around brackets"
    if re.search(r'[fF]["\']|\\["\']', diff) and re.search(r"[fF]\"|[fF]\'", diff):
        return "python gap: f-strings (implicit concatenation, quotes)"
    if '"""' in diff or "'''" in diff:
        return "python gap: multi-line string argument"
    if re.sub(r"\s+|[(),]", "", rout) == re.sub(r"\s+|[(),]", "", s_out):
        return "python gap: line breaks and parentheses"
    return "python gap: other"


def classify(work):
    work = Path(work)
    bad, per_file, project_of = pickle.loads((work / "bad.pkl").read_bytes())
    deciding, _ = evidence.split(evidence.files(mod), mod.SEED)
    by, files = Counter(), defaultdict(set)
    broken = defaultdict(set)
    for i, k, res, src, rout, s_out, r_py in bad:
        c = cause(res, src, rout, s_out, r_py)
        by[c] += 1
        files[c].add(i)
        broken[i].add(c)
    for c, n in by.most_common():
        print(f"{n:6d} cells {len(files[c]):4d} files  {c}")
    halves = {"all": set(per_file), "deciding": {i for i in per_file if project_of[i] in deciding}, "held out": {i for i in per_file if project_of[i] not in deciding}}
    order = [c for c, _ in sorted(((c, len(files[c])) for c in files), key=lambda t: -t[1])]
    print("cumulative: files whose cells are all equal after the causes are fixed, in order of files")
    for step in range(len(order) + 1):
        fixed = set(order[:step])
        row = []
        for h, fs in halves.items():
            ok = sum(1 for i in fs if broken[i] <= fixed)
            row.append(f"{h} {ok}/{len(fs)} = {100 * ok / len(fs):.1f}%")
        print(f"  {'now' if step == 0 else '+ ' + order[step - 1]}: " + "; ".join(row))


if __name__ == "__main__":
    cmd = sys.argv[1]
    if cmd == "format":
        format_baseline(*sys.argv[2:5])
    elif cmd == "lint":
        lint_baseline(*sys.argv[2:5])
    elif cmd == "classify":
        classify(sys.argv[2])
