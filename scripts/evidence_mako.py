"""Facts of the Mako format evidence (docs/ai/design/mako.md) for scripts/evidence.py: corpus, the rows of the decision table.

`python3 -I scripts/evidence.py mako table` prints the decision table with the vote of the deciding projects and the
held-out agreement of the form each row names in SUDSY.

Mako has four syntaxes of its own (docs/ai/design/mako.md, Mako's syntax.rst): `${ expr | filters }`, control lines
(`%` at the start of a line, `% endfor` closes a block) that share their form with `##` comment lines, `<% python %>` blocks
and `<%tag>` pairs. The tokenizer here is line-level and approximate, written from mako/lexer.py (the control-line regex);
the Mako compiler is the validity reference (scripts/evidence_mako_checks.py).
"""
import re

SEED = 20261204
NAMES = re.compile(r".*\.mako$")
DIRS = [("mako", NAMES)]
PROJECT = {}
CAP = 60
KEEP_EOL = True
TOOL = None
KLASSES = {}

# row -> the variant (or variants) a formatter writes; None: kept as written (below 60% of the deciding projects, or not a
# layout choice). Set from the vote of the deciding half (`evidence.py mako table`, one vote per project; the rule of
# no-reference-tool.md), then checked on the held-out half.
SUDSY = {
    "indent of a `%` control line": None,  # 54 indented, 47 none: below 60%
    "indent of a `%` line nested in a block": "same",  # 90 of 91: control lines stay flush with their opener
    "indent of a text line in a `%` block": "deeper",  # 62 of 91 (68%): default with an option for the other
    "space after `%` of a control line": "one",  # 63 of 101 (62%): default with an option for `%for`
    "space after `##` of a comment": "one",  # 46 of 47
    "space after `${`": "none",  # 375 of 379
    "space before `}` of `${`": "none",  # 375 of 379
    "space after `<%` of a Python block": "newline",  # 54 of 66
    "space before `%>` of a Python block": "newline",  # 54 of 66
    "tab in the indent of a line": "space",  # 356 of 362
    "blank lines in a row": "one",  # 391 of 396
    "blank lines at the start of a file": "none",  # 412 of 415
    "blank lines at the end of a file": "none",  # 394 of 415
    "trailing blanks of a line": "none",  # 415 of 415
    "final newline": "yes",  # 390 of 415
    "line endings": "LF",  # 410 of 415
}

# `%` or `##` at the start of a line (mako/lexer.py match_control_line); `%%` is an escaped text line (match_percent).
CONTROL = re.compile(r"^([ \t]*)(%(?!%)|##)([ \t]*)(.*)$")
KEYWORD = re.compile(r"(end)?(\w+)")
MIDDLE = {"else", "elif", "except", "finally"}  # continue a block; not an opener
PYBLOCK = re.compile(r"<%(\s+)(.*?)(\s*)%>", re.S)


def gap(s):
    """The blanks at one place: none, one space, a tab, several, or a line break."""
    if not s:
        return "none"
    if "\n" in s or "\r" in s:
        return "newline"
    if "\t" in s:
        return "tab"
    return "one" if s == " " else "several"


def bodies(text):
    """The text of each `${ }` expression, filters included; the end is the first `}` outside a string (as Mako's lexer)."""
    out, i = [], 0
    while True:
        i = text.find("${", i)
        if i < 0:
            return out
        j, quote = i + 2, None
        while j < len(text):
            c = text[j]
            if quote:
                if c == "\\":
                    j += 2
                    continue
                if c == quote:
                    quote = None
            elif c in "'\"":
                quote = c
            elif c == "}":
                break
            j += 1
        if j >= len(text):
            return out
        if text[i + 2 : j].strip():
            out.append(text[i + 2 : j])
        i = j + 1


def expressions(text):
    """[(gap after `${`, gap before `}`)] of each expression."""
    out = []
    for body in bodies(text):
        out.append((gap(body[: len(body) - len(body.lstrip())]), gap(body[len(body.rstrip()) :])))
    return out


def variants(text):
    v = {k: [] for k in SUDSY}
    raw = [l.rstrip("\r") for l in text.split("\n")]
    v["final newline"].append("yes" if text.endswith("\n") else "no")
    v["line endings"].append("CRLF" if "\r\n" in text else "LF")
    v["blank lines at the start of a file"].append("present" if raw and not raw[0].strip() else "none")
    v["blank lines at the end of a file"].append("present" if len(raw) > 1 and not raw[-1].strip() and not raw[-2].strip() else "none")
    for lo, hi in expressions(text):
        v["space after `${`"].append(lo)
        v["space before `}` of `${`"].append(hi)
    for lo, hi in ((gap(m.group(1)), gap(m.group(3))) for m in PYBLOCK.finditer(text)):
        v["space after `<%` of a Python block"].append(lo)
        v["space before `%>` of a Python block"].append(hi)
    stack, run, started = [], 0, False  # stack: indents of the open blocks
    for l in raw:
        if not l.strip():
            run += 1
            continue
        if run and started:
            v["blank lines in a row"].append({1: "one", 2: "two"}.get(run, "three or more"))
        run = 0
        started = True
        if l != l.rstrip():
            v["trailing blanks of a line"].append("present")
        else:
            v["trailing blanks of a line"].append("none")
        ind = l[: len(l) - len(l.lstrip(" \t"))]
        if ind:
            v["tab in the indent of a line"].append("tab" if "\t" in ind else "space")
        m = CONTROL.match(l)
        if m:
            _, op, g, rest = m.groups()
            if op == "##":
                if rest:
                    v["space after `##` of a comment"].append(gap(g))
                continue
            if not rest:
                continue
            v["indent of a `%` control line"].append("indented" if ind else "none")
            v["space after `%` of a control line"].append(gap(g))
            k = KEYWORD.match(rest)
            if k and k.group(1):  # `% end...` closes the innermost block
                if stack:
                    v["indent of a `%` line nested in a block"].append(cmp_indent(ind, stack[-1]))
                    stack.pop()
                continue
            if stack:
                v["indent of a `%` line nested in a block"].append(cmp_indent(ind, stack[-1]))
            if k and k.group(2) in MIDDLE:
                continue
            if rest.rstrip().endswith(":"):
                stack.append(ind)
            continue
        if stack and not l.lstrip().startswith("<%") and not l.lstrip().startswith("</%"):
            v["indent of a text line in a `%` block"].append(cmp_indent(ind, stack[-1]))
    return v


def cmp_indent(ind, opener):
    """A line's indent against the indent of the block's opener."""
    if len(ind) > len(opener):
        return "deeper"
    if len(ind) == len(opener):
        return "same"
    return "shallower"
