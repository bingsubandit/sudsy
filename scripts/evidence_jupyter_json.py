"""JSON side of the Jupyter plan (docs/ai/design/jupyter.md): splice new cell sources into the original text, byte for byte elsewhere, and compare with what
`ruff format` writes. A measuring script, not product code; `splice` is the model of the fix mapping.

Usage: evidence_jupyter_json.py <work-dir>   (after `evidence_jupyter_baseline.py format`: reads <work-dir>/orig and <work-dir>/rfmt)
"""
import json
import re
import sys
import time
from collections import Counter
from pathlib import Path

TOKEN = re.compile(r'"(?:[^"\\]|\\.)*"|[\[\]{}]')
WS = re.compile(r"\s*")
STR = re.compile(r'"(?:[^"\\]|\\.)*"')


def skip_value(t, i):
    """End of the JSON value that starts at `i`."""
    i = WS.match(t, i).end()
    c = t[i]
    if c == '"':
        return STR.match(t, i).end()
    if c in "[{":
        depth = 0
        for m in TOKEN.finditer(t, i):
            g = m.group()
            if g in "[{":
                depth += 1
            elif g in "]}":
                depth -= 1
                if depth == 0:
                    return m.end()
        raise ValueError("unterminated")
    m = re.compile(r"[^,\]}\s]+").match(t, i)
    return m.end()


def members(t, i):
    """(key, key start, value start, value end) of the object at `i`."""
    i = WS.match(t, i).end()
    assert t[i] == "{"
    i += 1
    while True:
        i = WS.match(t, i).end()
        if t[i] == "}":
            return
        k = STR.match(t, i)
        ks = i
        i = WS.match(t, k.end()).end()
        assert t[i] == ":"
        vs = WS.match(t, i + 1).end()
        ve = skip_value(t, vs)
        yield json.loads(k.group()), ks, vs, ve
        i = WS.match(t, ve).end()
        if t[i] == ",":
            i += 1


def elements(t, i):
    i = WS.match(t, i).end()
    assert t[i] == "["
    i += 1
    while True:
        i = WS.match(t, i).end()
        if t[i] == "]":
            return
        e = skip_value(t, i)
        yield i, e
        i = WS.match(t, e).end()
        if t[i] == ",":
            i += 1


def source_spans(t):
    """[(key start, value start, value end)] of every cell's `source`, in cell order."""
    out = []
    for k, _, vs, ve in members(t, 0):
        if k == "cells":
            for cs, ce in elements(t, vs):
                span = None
                for ck, cks, cvs, cve in members(t, cs):
                    if ck == "source":
                        span = (cks, cvs, cve)
                out.append(span)
    return out


def encode(lines, t, key_start, unit, ascii_only):
    """The `source` value of a changed cell: a list of lines, laid out as the file lays out its other arrays."""
    ls = t.rfind("\n", 0, key_start) + 1
    base = re.match(r"[ \t]*", t[ls:key_start]).group()
    items = [json.dumps(x, ensure_ascii=ascii_only) for x in lines]
    if not items:
        return "[]"
    return "[\n" + ",\n".join(base + unit + x for x in items) + "\n" + base + "]"


def to_lines(s):
    return s.splitlines(keepends=True) if s else []


def splice(t, new_sources):
    """The text `t` with the sources of the cells whose text differs from `new_sources` replaced; everything else byte for byte."""
    spans = source_spans(t)
    cells = json.loads(t)["cells"]
    first = re.search(r'\n([ \t]*)"', t)
    unit = first.group(1) if first else " "
    ascii_only = not re.search(r"[^\x00-\x7f]", t) and "\\u" in t
    pieces, last = [], 0
    for span, cell, new in zip(spans, cells, new_sources):
        if span is None or new is None:
            continue
        old = cell["source"]
        old = "".join(old) if isinstance(old, list) else old
        if old == new:
            continue
        ks, vs, ve = span
        pieces.append(t[last:vs])
        pieces.append(encode(to_lines(new), t, ks, unit, ascii_only))
        last = ve
    pieces.append(t[last:])
    return "".join(pieces)


def features(orig, canon):
    f = []
    m = re.match(r'\{\r?\n([ \t]*)"', orig)
    if not m or m.group(1) != " ":
        f.append("indent is not one space")
    nb = json.loads(orig)
    if list(nb) != sorted(nb) or any(isinstance(c, dict) and list(c) != sorted(c) for c in nb["cells"]) or re.sub(r"\s", "", orig) != re.sub(r"\s", "", json.dumps(nb, sort_keys=True, ensure_ascii=False)):
        f.append("keys not sorted or escapes differ")
    if "\\u" in orig and re.search(r"\\u[0-9a-fA-F]{4}", orig):
        f.append("\\uXXXX escapes (ruff writes the character)")
    if nb.get("nbformat_minor", 0) >= 5 and any("id" not in c for c in nb["cells"]):
        f.append("cell ids missing (ruff adds them)")
    if "\r\n" in orig:
        f.append("CRLF line ends")
    return f


def main(work):
    work = Path(work)
    cls, files = Counter(), {}
    t_splice = 0.0
    n_changed = 0
    for rp in sorted((work / "rfmt").glob("*.ipynb")):
        op = work / "orig" / rp.name
        o, r = op.read_text(encoding="utf-8"), rp.read_text(encoding="utf-8")
        if o == r:
            cls["ruff leaves the file as it is"] += 1
            continue
        n_changed += 1
        rn = json.loads(r)
        t0 = time.perf_counter()
        model = splice(o, [("".join(c["source"]) if isinstance(c["source"], list) else c["source"]) for c in rn["cells"]])
        t_splice += time.perf_counter() - t0
        if model == r:
            cls["ruff changes cells; the splice is byte-identical to ruff's file"] += 1
            continue
        feats = features(o, r)
        if "ids" in " ".join(feats) and len(feats) == 1:
            pass
        key = "ruff changes cells and the layout: " + (" + ".join(feats) if feats else "other")
        cls[key] += 1
        files.setdefault(key, []).append(rp.name)
        # the content that ruff changed besides sources and ids
        on, mn = json.loads(o), json.loads(model)
        strip = lambda nb: ([{k: v for k, v in c.items() if k not in ("source", "id")} for c in nb["cells"]], {k: v for k, v in nb.items() if k != "cells"})
        if strip(on) != strip(rn):
            cls["  of which ruff changed values other than source and id"] += 1
            files.setdefault("values", []).append(rp.name)
    for k, v in cls.most_common():
        print(f"{v:5d}  {k}")
    print(f"splice time over {n_changed} changed files: {t_splice:.2f} s")
    for k, v in files.items():
        print(k, v[:5])


if __name__ == "__main__":
    main(sys.argv[1])
