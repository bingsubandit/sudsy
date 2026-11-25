"""Facts of the Jupyter evidence (docs/ai/design/jupyter.md) for scripts/evidence.py: corpus, split, JSON layout rows of a notebook file.

The cell comparison with ruff is scripts/evidence_jupyter_cells.py (it reads the corpus through this module).
"""
import json
import re
from collections import Counter

SEED = 20261202
CAP = 40
DIRS = [("jupyter", re.compile(r"^.*\.ipynb$"))]
PROJECT = {"docs": "tensorflow-docs", "notebooks": "huggingface-notebooks", "tutorials": "pytorch-tutorials", "interactive": "dotnet-interactive"}
KLASSES = {}
KEEP_EOL = True

# rows of the JSON layout of a file; all kept as written: the writer of the file (Jupyter, VS Code, ruff) decides, and the tools agree on that
SUDSY = {
    "final newline (per file)": None,
    "indent (per file)": None,
    "non-ASCII written as escapes (per file)": None,
    "source as list of lines (per cell)": None,
    "key order sorted (per file)": None,
}


def indent_of(text):
    m = re.match(r'\{\r?\n( *|\t)"', text)
    return "compact" if m is None and "\n" not in text[:200] else ("tab" if m and m.group(1) == "\t" else f"{len(m.group(1))} spaces" if m else "other")


def variants(text):
    rows = {r: [] for r in SUDSY}
    rows["final newline (per file)"].append("yes" if text.endswith("\n") else "no")
    rows["indent (per file)"].append(indent_of(text))
    rows["non-ASCII written as escapes (per file)"].append("escaped" if re.search(r"\\u[0-9a-fA-F]{4}", text) and not re.search(r"[^\x00-\x7f]", text) else "raw" if re.search(r"[^\x00-\x7f]", text) else "ascii only")
    try:
        nb = json.loads(text)
    except ValueError:
        return rows
    for c in nb.get("cells", []):
        rows["source as list of lines (per cell)"].append("list" if isinstance(c.get("source"), list) else "string")
    keys = list(nb)
    rows["key order sorted (per file)"].append("sorted" if keys == sorted(keys) else "other")
    return rows
