"""Cell-level evidence of the Jupyter plan (docs/ai/design/jupyter.md): corpus statistics and the baseline comparison with ruff.

Usage: evidence_jupyter_cells.py stats
       evidence_jupyter_cells.py baseline <sudsy-binary> <ruff-binary> <work-dir>
The corpus list and the project split come from scripts/evidence.py and scripts/evidence_jupyter.py; nothing here is product code.
"""
import ast
import warnings
import json
import re
import statistics
import sys
from collections import Counter
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import evidence  # noqa: E402
import evidence_jupyter as mod  # noqa: E402

warnings.simplefilter("ignore")
MAGIC_LINE = re.compile(r"^\s*(%{1,2}\w|!|\?|\w[\w.]*\?{1,2}\s*$)")
CELL_MAGIC = re.compile(r"^\s*%%\w")


def source(cell):
    s = cell.get("source", "")
    return "".join(s) if isinstance(s, list) else s


def load(f):
    try:
        return json.loads(f.read_text(encoding="utf-8"))
    except (ValueError, UnicodeDecodeError):
        return None


def kernel_lang(nb):
    md = nb.get("metadata", {}) if isinstance(nb.get("metadata"), dict) else {}
    ks, li = md.get("kernelspec", {}), md.get("language_info", {})
    ks, li = ks if isinstance(ks, dict) else {"name": ks}, li if isinstance(li, dict) else {}
    return (li.get("name") or ks.get("language") or ks.get("name") or "none").lower()


def strip_magics(src):
    """Magic lines blanked: what is left must be Python (the way nbQA and ruff set them aside)."""
    out = []
    for l in src.split("\n"):
        out.append("pass" if MAGIC_LINE.match(l) else l)
    return "\n".join(out)


def parses(src):
    try:
        ast.parse(src)
        return True
    except (SyntaxError, ValueError, RecursionError):
        return False


def stats():
    corpus = evidence.files(mod)
    deciding, held = evidence.split(corpus, mod.SEED)
    nbf, langs, sizes, outshare = Counter(), Counter(), [], []
    cells = Counter()
    n_bad = 0
    magic = Counter()
    py_cells = 0
    bad_raw = bad_stripped = 0
    bad_files = set()
    magic_files = set()
    per_project = Counter()
    for proj, name, f in corpus:
        nb = load(f)
        if nb is None:
            n_bad += 1
            continue
        per_project[proj] += 1
        nbf[f"{nb.get('nbformat')}.{nb.get('nbformat_minor')}" if "nbformat" in nb else "none (v3 worksheets)" if "worksheets" in nb else "none"] += 1
        lang = kernel_lang(nb)
        langs[lang] += 1
        if lang in ("python3", "ipython", "none"):
            lang = "python"  # no kernel information is read as Python (ruff: checked in the plan)
        size = f.stat().st_size
        sizes.append(size)
        out_bytes = sum(len(json.dumps(c.get("outputs", []))) for c in nb.get("cells", []))
        outshare.append(out_bytes / max(size, 1))
        if lang != "python":
            for c in nb.get("cells", []):
                cells["other-kernel " + c.get("cell_type", "?")] += 1
            continue
        for c in nb.get("cells", []):
            t = c.get("cell_type", "?")
            cells[t] += 1
            if t != "code":
                continue
            py_cells += 1
            s = source(c)
            ls = s.split("\n")
            kinds = set()
            for l in ls:
                m = MAGIC_LINE.match(l)
                if m:
                    g = m.group(1)
                    kinds.add("cell magic %%" if g.startswith("%%") else "line magic %" if g.startswith("%") else "shell !" if g == "!" else "help ?")
            if kinds:
                magic_files.add(name)
            for k in kinds:
                magic[k] += 1
            if kinds:
                magic["any"] += 1
            if CELL_MAGIC.match(ls[0] if ls else ""):
                continue  # a cell magic body is not Python (%%bash, %%sql, %%time is)
            if not parses(s):
                bad_raw += 1
                if not parses(strip_magics(s)):
                    bad_stripped += 1
                    bad_files.add(name)
    print(f"{len(corpus)} files of {len(set(p for p, _, _ in corpus))} projects, {n_bad} not JSON; {len(deciding)} deciding / {len(held)} held-out projects")
    print("nbformat:", dict(nbf.most_common()))
    print("kernel language (files):", dict(langs.most_common()))
    print("cells (python kernels, plus other kernels' cells):", dict(cells.most_common()))
    print(f"code cells of python kernels: {py_cells}; with a magic line: {magic['any']} ({100 * magic['any'] / max(py_cells, 1):.1f}%) in {len(magic_files)} files; by kind: { {k: v for k, v in magic.items() if k != 'any'} }")
    print(f"code cells not parsing as Python (CPython ast, cell magic bodies left out): raw {bad_raw} ({100 * bad_raw / max(py_cells, 1):.1f}%), after setting magic lines aside {bad_stripped} ({100 * bad_stripped / max(py_cells, 1):.2f}%) in {len(bad_files)} files")
    q = statistics.quantiles(sizes, n=100)
    print(f"file size: total {sum(sizes) / 1e6:.1f} MB, median {statistics.median(sizes) / 1e3:.1f} KB, p90 {q[89] / 1e3:.0f} KB, p99 {q[98] / 1e3:.0f} KB, max {max(sizes) / 1e6:.2f} MB; files over 1 MB {sum(s > 1e6 for s in sizes)}, over 5 MB {sum(s > 5e6 for s in sizes)}")
    print(f"outputs share of the bytes: median {100 * statistics.median(outshare):.0f}%, mean {100 * statistics.mean(outshare):.0f}%; files with over 50% outputs {sum(o > 0.5 for o in outshare)}")


if __name__ == "__main__":
    {"stats": stats}[sys.argv[1]]()
