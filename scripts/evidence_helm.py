"""Facts of the Helm template evidence (docs/ai/design/helm.md) for scripts/evidence.py: corpus, the rows of the decision table.

`python3 -I scripts/evidence.py helm table` prints the decision table with the vote of the deciding projects and the
held-out agreement of the form each row names in SUDSY. No formatter exists for Helm templates (docs/ai/design/helm.md),
so the rows are the layout choices of the templates as written, read from the corpus alone.

Helm templates are Go templates (text/template) in YAML: actions `{{ ... }}` with a left trim `{{-` and a right trim `-}}`,
block words (`if`, `range`, `with`, `define`, `block`; `else`, `elif`; `end`), and comments `{{/* */}}`. The tokenizer here is
line-level and approximate, written from Go's text/template lexer (an action ends at the first `}}` outside a quoted string;
a trim marker is a `-` next to a space); the corpus is the reference, and the Helm tools are the validity reference
(scripts/evidence_helm_tools.sh).

Corpus: tests/conformance/corpus/helm (fetch: tests/conformance/fetch-helm-corpus.sh). Only files under a `templates` folder
are read (a chart's templates; Chart.yaml and values.yaml are the tools' input, not the format's). A project keeps at most
CAP files, the first by path.
"""
import bisect
import re

SEED = 20261204
DIRS = [("helm", re.compile(r"^.*\.(ya?ml|tpl)$"))]
PROJECT = {}
CAP = 40
KEEP_EOL = True
TOOL = None
KLASSES = {}

# row -> the variant (or variants) a formatter writes; None: kept as written. Set from the vote of the deciding half (one vote
# per project), then checked on the held-out half. Trims and the `indent`/`nindent` filter change the rendered output, and
# an action's line is part of the YAML text: those stay as written whatever the vote (docs/ai/design/helm.md).
SUDSY = {
    "indent of a control line against the content it opens": "same",  # 199 of 224 projects
    "indent of an end line against its opener": "same",  # 224 of 224
    "control action on its own line": None,  # 222 of 225 own line, but moving an action moves its whitespace
    "left trim of a control action (`{{-`)": None,  # 215 of 225 trim: a trim changes the rendered text
    "right trim of a control action (`-}}`)": None,  # 215 of 225 plain: a trim changes the rendered text
    "space after `{{` of an action": "one",  # 300 of 302
    "space before `}}` of an action": "one",  # 299 of 302
    "indent filter (`indent` or `nindent`)": None,  # 176 of 207 nindent: the filter changes the rendered text
    "YAML indent step of a host line": "2",  # 289 of 293
    "tab in the indent of a line": "space",  # 299 of 299
    "blank lines in a row": "one",  # 76 of 77
    "blank lines at the start of a file": "none",  # 302 of 302
    "blank lines at the end of a file": "none",  # 291 of 302
    "trailing blanks of a line": "none",  # 302 of 302
    "final newline": "yes",  # 247 of 302
    "line endings": "LF",  # 299 of 302
}

BLOCK_OPEN = {"if", "range", "with", "define", "block"}
BLOCK_END = {"end"}
FILTER = re.compile(r"\|\s*(n?indent)\s+\d+")
WORD = re.compile(r"(\w+)")


def KEEP(rel):
    """A file of a chart's templates folder (the first part of `rel` is the project folder)."""
    return "templates" in rel.parts


COMMENT = re.compile(r"[-\s]*/\*")


def actions(text):
    """[(start, end, body)] of each action: `{{` up to the first `}}` outside a quoted string (text/template's lexer); a
    comment (`{{/* */}}`, trims allowed) runs to its `*/`, and quotes in it do not count."""
    out, i = [], 0
    while True:
        i = text.find("{{", i)
        if i < 0:
            return out
        s = i + 2
        if COMMENT.match(text, s):
            close = text.find("*/", s)
            j = text.find("}}", close + 2) if close >= 0 else -1
            if j < 0:
                return out
        else:
            j, quote = s, None
            while j < len(text):
                c = text[j]
                if quote:
                    if c == "\\" and quote == '"':
                        j += 2
                        continue
                    if c == quote:
                        quote = None
                elif c in "\"'`":
                    quote = c
                elif text.startswith("}}", j):
                    break
                j += 1
            if j >= len(text):
                return out
        out.append((i, j + 2, text[s:j]))
        i = j + 2


def gap(s):
    """The blanks at one place: none, one space, a tab, several, or a line break."""
    if not s:
        return "none"
    if "\n" in s or "\r" in s:
        return "newline"
    if "\t" in s:
        return "tab"
    return "one" if s == " " else "several"


def trims(body):
    """(left trim, right trim, gap after `{{`, gap before `}}`): a trim is a `-` with a blank on its outer side."""
    lt = len(body) > 1 and body[0] == "-" and body[1].isspace()
    rt = len(body) > 1 and body[-1] == "-" and body[-2].isspace()
    inner = body[1:] if lt else body
    inner = inner[:-1] if rt else inner
    lead = inner[: len(inner) - len(inner.lstrip())]
    tail = inner[len(inner.rstrip()) :]
    return lt, rt, gap(lead), gap(tail), inner.strip()


def cmp_indent(ind, other):
    """A line's indent against another line's indent."""
    if len(ind) > len(other):
        return "deeper"
    if len(ind) == len(other):
        return "same"
    return "shallower"


def indent_of(line):
    return line[: len(line) - len(line.lstrip(" \t"))]


def variants(text):
    v = {k: [] for k in SUDSY}
    raw = [l.rstrip("\r") for l in text.split("\n")]
    v["final newline"].append("yes" if text.endswith("\n") else "no")
    v["line endings"].append("CRLF" if "\r\n" in text else "LF")
    nonblank = [k for k, l in enumerate(raw) if l.strip()]
    v["blank lines at the start of a file"].append("present" if nonblank and nonblank[0] > 0 else "none")
    v["blank lines at the end of a file"].append("present" if len(raw) > 2 and not raw[-1].strip() and not raw[-2].strip() else "none")
    run = 0
    for k, l in enumerate(raw):
        if not l.strip():
            run += 1
            continue
        if run and k:
            v["blank lines in a row"].append({1: "one", 2: "two"}.get(run, "three or more"))
        run = 0
        ind = indent_of(l)
        if ind:
            v["tab in the indent of a line"].append("tab" if "\t" in ind else "space")
        v["trailing blanks of a line"].append("present" if l != l.rstrip() else "none")
    # host lines: the YAML indent step, the smallest step from a line to a deeper one (lines without an action)
    host = [indent_of(l) for l in raw if l.strip() and "{{" not in l]
    steps = [len(b) - len(a) for a, b in zip(host, host[1:]) if len(b) > len(a)]
    if steps:
        s = min(steps)
        v["YAML indent step of a host line"].append({2: "2", 4: "4"}.get(s, "other"))
    # actions: positions of their lines, the stack of open blocks (indent of an own-line opener, or None)
    starts = [0] + [m + 1 for m in (i for i, c in enumerate(text) if c == "\n")]
    stack = []
    for a, b, body in actions(text):
        if body.lstrip().startswith("/*"):
            continue
        ln = bisect.bisect_right(starts, a) - 1
        lt, rt, g_open, g_close, inner = trims(body)
        v["space after `{{` of an action"].append(g_open)
        v["space before `}}` of an action"].append(g_close)
        for m in FILTER.finditer(body):
            v["indent filter (`indent` or `nindent`)"].append(m.group(1))
        word = WORD.match(inner)
        kw = word.group(1) if word else ""
        if kw not in BLOCK_OPEN and kw not in BLOCK_END and kw not in ("else", "elif"):
            continue
        own = raw[ln].strip() == text[a:b] if ln < len(raw) else False
        v["control action on its own line"].append("own line" if own else "shared line")
        v["left trim of a control action (`{{-`)"].append("trim" if lt else "plain")
        v["right trim of a control action (`-}}`)"].append("trim" if rt else "plain")
        ind = indent_of(raw[ln]) if own else None
        if kw in BLOCK_OPEN:
            # the content it opens: the next line that is not an action line
            nxt = next((raw[j] for j in range(ln + 1, len(raw)) if raw[j].strip() and not raw[j].lstrip().startswith("{{")), None)
            if ind is not None and nxt is not None:
                v["indent of a control line against the content it opens"].append(cmp_indent(ind, indent_of(nxt)))
            stack.append(ind)
        elif kw in BLOCK_END:
            opener = stack.pop() if stack else None
            if ind is not None and opener is not None:
                v["indent of an end line against its opener"].append(cmp_indent(ind, opener))
    return v
