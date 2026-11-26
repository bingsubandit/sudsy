"""Lines of a notebook file for the lint parity runs: a tool reports a finding of a notebook as a row of a cell, Sudsy as a line of the file."""
import json
from functools import lru_cache

DECODER = json.JSONDecoder()


def skip(text, i):
    while text[i] in " \t\r\n":
        i += 1
    return i


def members(text, i):
    """`[(key, start of the value)]` of the object at `i`."""
    out, i = [], skip(text, i + 1)
    while text[i] != "}":
        key, i = DECODER.raw_decode(text, i)
        i = skip(text, i)
        i = skip(text, i + 1)
        out.append((key, i))
        _, i = DECODER.raw_decode(text, i)
        i = skip(text, i)
        if text[i] == ",":
            i = skip(text, i + 1)
    return out


def items(text, i):
    """`[start of each element]` of the array at `i`."""
    out, i = [], skip(text, i + 1)
    while text[i] != "]":
        out.append(i)
        _, i = DECODER.raw_decode(text, i)
        i = skip(text, i)
        if text[i] == ",":
            i = skip(text, i + 1)
    return out


@lru_cache(maxsize=4)
def cell_sources(text):
    """`[[(decoded string, line of its token)]]` per cell: the pieces of each cell's `source`."""
    cells = dict(members(text, skip(text, 0)))["cells"]
    out = []
    for start in items(text, cells):
        value = dict(members(text, start)).get("source")
        if value is None:
            out.append([])
            continue
        starts = items(text, value) if text[value] == "[" else [value]
        out.append([(DECODER.raw_decode(text, s)[0], text.count("\n", 0, s) + 1) for s in starts])
    return out


def host_line(text, cell, row):
    """The line of the file that holds row `row` (1-based) of cell `cell` (1-based, as the tool numbers the cells of the notebook)."""
    pieces = cell_sources(text)[cell - 1]
    code = "".join(p for p, _ in pieces)
    at = 0
    for _ in range(row - 1):
        at = code.find("\n", at) + 1
    seen = 0
    for piece, line in pieces:
        seen += len(piece)
        if at < seen:
            return line
    return pieces[-1][1] if pieces else 1
