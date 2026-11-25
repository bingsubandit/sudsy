"""Facts of the Makefile format evidence (docs/ai/design/makefile.md) for scripts/evidence.py: corpus, the rows of the decision table, the causes of difference to mbake."""
import re
from difflib import SequenceMatcher

SEED = 20261127
NAMES = re.compile(r"^(Makefile|makefile|GNUmakefile|.*\.mk|.*\.mak)$")
DIRS = [("makefile", NAMES), ("makefile-evidence", NAMES)]
PROJECT = {"cli": "docker-cli"}
TOOL = ["format", "--stdin"]
KLASSES = {"indent": "indentation", "blank": "blanks inside lines and blank lines"}

ONE, SEVERAL = "one blank", "several blanks"
# row -> the variant (or variants) Sudsy writes; None: kept as written
SUDSY = {
    "blanks around an assignment operator": "one each side",
    "alignment of assignments": "not aligned",
    "blank before the colon of a rule": "none",
    "blank after the colon of a rule": ONE,
    "blanks between the words of a rule line": ONE,
    "recipe line indent": "one tab",
    "conditional body indent (outside a recipe)": "as written",
    "conditional in a recipe: lines": "column 0",
    "rule or directive continuation: blank before the backslash": ONE,
    "rule or directive continuation: indent": "as written",
    "assignment continuation: blank before the backslash": None,
    "assignment continuation: indent": None,
    "comment: blank after `#`": None,
    "blank lines": {"one", "two"},
    "trailing blanks in the file": "none",
    "final newline": "yes",
}

COND_OPEN = re.compile(r"[ ]*(ifeq|ifneq|ifdef|ifndef)\b")
COND_ELSE = re.compile(r"[ ]*else\b")
COND_END = re.compile(r"[ ]*endif\b")
DEFINE = re.compile(r"[ ]*(?:(?:export|override|private)\s+)*define\b")
ENDEF = re.compile(r"[ ]*endef\b")
DIRECTIVE = re.compile(r"[ ]*(?:-?include|sinclude|export|unexport|override|private|vpath|undefine)\b")
ASSIGN_OPS = {"=", ":=", "::=", ":::=", "+=", "?=", "!="}


def mask(s):
    """The line with every `$(...)`, `${...}`, `$$` and `$x` replaced by `X`s of the same length."""
    out, i = [], 0
    while i < len(s):
        if s[i] != "$" or i + 1 >= len(s):
            out.append(s[i])
            i += 1
            continue
        if s[i + 1] in "({":
            close = ")" if s[i + 1] == "(" else "}"
            depth, j = 1, i + 2
            while j < len(s) and depth:
                depth += (s[j] == s[i + 1]) - (s[j] == close)
                j += 1
            out.append("X" * (j - i))
            i = j
        else:
            out.append("XX")
            i += 2
    return "".join(out)


def code(s):
    """The masked first line without its comment and its continuation backslash."""
    m = mask(s)
    h = m.find("#")
    if h >= 0:
        m = m[:h]
    return m.rstrip()[:-1].rstrip() if m.rstrip().endswith("\\") else m.rstrip()


def split_statements(text):
    """[(kind, physical lines, in a recipe context before it)]; kind is blank, comment, recipe, cond-open, cond-else, cond-end,
    define, directive, assign, rule or other. The body of a define is not listed. A recipe context is what a conditional
    after a rule is in: the formatter reads its lines as part of the recipe."""
    lines = text.split("\n")
    if lines and lines[-1] == "":
        lines.pop()
    lines = [l.rstrip("\r") for l in lines]
    out, i, in_rule, define = [], 0, False, 0
    while i < len(lines):
        l = lines[i]
        if define:
            define += bool(DEFINE.match(l)) - bool(ENDEF.match(l))
            i += 1
            continue
        phys = [l]
        while phys[-1].endswith("\\") and (len(phys[-1]) - len(phys[-1].rstrip("\\"))) % 2 and i + len(phys) < len(lines):
            phys.append(lines[i + len(phys)])
        i += len(phys)
        s = code(l)
        if not l.strip():
            kind = "blank"
        elif l.startswith("\t") and in_rule:
            kind = "recipe"
        elif l.lstrip().startswith("#"):
            kind = "comment"
        elif COND_OPEN.match(l):
            kind = "cond-open"
        elif COND_ELSE.match(l):
            kind = "cond-else"
        elif COND_END.match(l):
            kind = "cond-end"
        elif DEFINE.match(l):
            kind, define = "define", 1
        else:
            kind = "assign" if assignment(s) else "directive" if DIRECTIVE.match(l) else "rule" if ":" in s else "other"
        out.append((kind, phys, in_rule))
        if kind in ("rule", "recipe"):
            in_rule = True
        elif kind not in ("blank", "comment", "cond-open", "cond-else", "cond-end"):
            in_rule = False
    return out


def assignment(s):
    """(column of the operator, blanks before it, the operator, blanks after it or None without a value), or None."""
    i = s.find("=")
    if i < 0 or ";" in s[:i]:
        return None
    j = i
    while j > 0 and s[j - 1] in ":+?!":
        j -= 1
    while j < i and s[j : i + 1] not in ASSIGN_OPS:
        j += 1
    head = s[:j]
    name = head.strip()
    if not name or name.split()[0] in ("ifeq", "ifneq", "ifdef", "ifndef", "else") or "=" in name:
        return None
    rest = s[i + 1 :]
    return j, head[len(head.rstrip(" \t")) :], s[j : i + 1], rest[: len(rest) - len(rest.lstrip(" \t"))] if rest.strip() else None


def op_variant(pre, post):
    if pre == " " and post in (" ", None):
        return "one each side"
    if pre == "" and post in ("", None):
        return "none"
    if pre in ("", " ") and post in ("", " "):
        return "blank before only" if pre else "blank after only"
    return SEVERAL


def blanks(g):
    return "none" if g == "" else ONE if g == " " else SEVERAL


def indent_name(g):
    if g == "":
        return "none"
    if set(g) == {" "}:
        return f"{len(g)} spaces"
    return "tab" if g == "\t" else "tabs and spaces"


def variants(text):
    rows = {r: [] for r in SUDSY}
    rows["final newline"].append("yes" if text.endswith("\n") else "no")
    stack = []  # per open conditional: is it in a recipe context
    group, run, trail = [], 0, False

    def flush():
        if len(group) >= 2:
            padded = any(len(pre) > 1 or "\t" in pre for _, pre in group)
            rows["alignment of assignments"].append("aligned" if len({c for c, _ in group}) == 1 and padded else "not aligned")
        group.clear()

    for kind, phys, was_in_rule in split_statements(text):
        if kind == "blank":
            run += 1
            flush()
            continue
        if run:
            rows["blank lines"].append("one" if run == 1 else "two" if run == 2 else "three or more")
            run = 0
        first = phys[0]
        trail = trail or any(p != p.rstrip(" \t") for p in phys)
        if kind != "assign":
            flush()
        if kind == "cond-open":
            stack.append(was_in_rule or bool(stack and stack[-1]))
        if kind in ("cond-open", "cond-else", "cond-end", "assign", "rule", "directive", "define", "other") and stack:
            if stack[-1]:
                rows["conditional in a recipe: lines"].append("column 0" if not first.startswith((" ", "\t")) else "indented")
            else:
                depth = len(stack) - (kind in ("cond-open", "cond-else", "cond-end"))
                if depth >= 1:
                    rows["conditional body indent (outside a recipe)"].append(level(first, depth))
        if kind == "cond-end" and stack:
            stack.pop()
        if kind == "comment":
            m = re.match(r"\s*#(\s|[^\s#!=\-*~_/+.<>|])", first)
            if m:
                rows["comment: blank after `#`"].append("one blank" if m.group(1).isspace() else "none")
        if kind == "assign":
            col, pre, op, post = assignment(code(first))
            rows["blanks around an assignment operator"].append(op_variant(pre, post))
            group.append((col, pre))
        if kind == "rule":
            rule_rows(rows, first)
        if kind == "recipe":
            rest = first[1:]
            rows["recipe line indent"].append("tab plus blanks" if rest[:1] in (" ", "\t") else "one tab")
        if len(phys) > 1 and kind in ("rule", "directive", "assign", "cond-open", "define"):
            cont_rows(rows, kind, phys)
    flush()
    rows["trailing blanks in the file"].append("has" if trail else "none")
    return rows


def level(first, depth):
    g = first[: len(first) - len(first.lstrip(" \t"))]
    if g == "":
        return "none"
    if set(g) == {" "} and len(g) % depth == 0:
        return f"{len(g) // depth} spaces per level"
    if set(g) == {"\t"} and len(g) == depth:
        return "tab per level"
    return "other"


def rule_rows(rows, first):
    s = code(first)
    m = re.search(r"::?(?!=)", s)
    if not m or s[: m.start()].endswith("&"):
        return
    head, tail = s[: m.start()], s[m.end() :]
    if head.strip():
        rows["blank before the colon of a rule"].append(blanks(head[len(head.rstrip(" \t")) :]))
    semi = tail.find(";")
    if semi >= 0:
        tail = tail[:semi]
    if tail.strip() and not tail.strip().startswith("|"):
        rows["blank after the colon of a rule"].append(blanks(tail[: len(tail) - len(tail.lstrip(" \t"))]))
    words = (head.strip() + " " + tail.strip()).split()
    if len(words) >= 2:
        gaps = re.findall(r"(?<=\S)([ \t]+)(?=\S)", head.strip()) + re.findall(r"(?<=\S)([ \t]+)(?=\S)", tail.strip())
        rows["blanks between the words of a rule line"].append(SEVERAL if any(g != " " for g in gaps) else ONE)


def cont_rows(rows, kind, phys):
    rule = kind != "assign"
    pre = "rule or directive" if rule else "assignment"
    for k, p in enumerate(phys):
        if k + 1 < len(phys) and p.strip() != "\\":
            body = p[:-1]
            rows[f"{pre} continuation: blank before the backslash"].append(blanks(body[len(body.rstrip(" \t")) :]))
        if k:
            rows[f"{pre} continuation: indent"].append(indent_name(p[: len(p) - len(p.lstrip(" \t"))]))


def key(l):
    return "".join(l.split())


def causes(a, b):
    """[(cause, class)] of the differences between two outputs (a: Sudsy, b: mbake): the lines are matched by their text without
    blanks, so a line that differs only in blanks is one cause and an added, removed or rewritten line another."""
    x, y = a.split("\n"), b.split("\n")
    out = []

    def add(cause, cls, ex):
        EXAMPLES.setdefault(cause, []).append(ex)
        out.append((cause, cls))

    for tag, i1, i2, j1, j2 in SequenceMatcher(None, [key(l) for l in x], [key(l) for l in y], autojunk=False).get_opcodes():
        if tag == "equal":
            for k in range(i2 - i1):
                u, v = x[i1 + k], y[j1 + k]
                if u != v:
                    for cause, cls in line_causes(u, v, k and x[i1 + k - 1].endswith("\\")):
                        add(cause, cls, (u, v))
        elif all(not l.strip() for l in x[i1:i2] + y[j1:j2]):
            add("blank lines added or removed", "blank", ("\n".join(x[i1:i2]), "\n".join(y[j1:j2])))
        else:
            add("lines added, removed or rewritten", "", ("\n".join(x[i1:i2]), "\n".join(y[j1:j2])))
    return out


def line_causes(u, v, continued):
    """Why two lines with the same text without blanks differ."""
    ru, rv = u.lstrip(" \t"), v.lstrip(" \t")
    out = []
    if ru != rv:
        if ru.rstrip() == rv.rstrip():
            out.append(("trailing blanks", "blank"))
        elif assignment(code(u)) and assignment(code(v)):
            out.append(("blanks around an assignment operator", "blank"))
        elif ru.endswith("\\") and rv.endswith("\\") and ru[:-1].rstrip() == rv[:-1].rstrip():
            out.append(("blank before the backslash", "blank"))
        else:
            out.append(("blanks inside a line", "blank"))
    lu, lv = lead(u), lead(v)
    if lu != lv and ru.strip():
        if continued:
            out.append(("indent of a continued line", "indent"))
        elif lv.startswith("\t") and not lu.startswith("\t"):
            out.append(("indent of a recipe line (tab)", "indent"))
        elif lu.startswith(" ") and lv == "":
            out.append(("indent of a conditional body or a comment (Sudsy indents)", "indent"))
        elif lu == "" and lv:
            out.append(("indent (mbake indents, Sudsy does not)", "indent"))
        else:
            out.append(("indent: other", "indent"))
    return out


def lead(l):
    return l[: len(l) - len(l.lstrip(" \t"))]


def meaning(src, out):
    """[(line number, what)] where Sudsy's output may differ in meaning from the source: make reads text, not only tokens, so
    a change of blanks is checked by where it happens. Blank lines, indentation and the blanks around operators, colons and
    words of a rule line carry no meaning; the trailing blanks of an assignment's value and of a recipe command are text."""
    x, y = src.split("\n"), out.split("\n")
    if "".join(src.split()) != "".join(out.split()):
        return [(0, "non-blank text differs")]
    found = []
    for tag, i1, i2, j1, j2 in SequenceMatcher(None, [key(l) for l in x], [key(l) for l in y], autojunk=False).get_opcodes():
        if tag != "equal":
            continue
        for k in range(i2 - i1):
            u, v = x[i1 + k], y[j1 + k]
            if u.rstrip(" \t") != u and v == v.rstrip(" \t") and u.strip() and not u.lstrip().startswith("#"):
                kind = "a recipe command" if u.startswith("\t") else "an assignment value" if assignment(code(u)) else "another line"
                found.append((i1 + k + 1, f"trailing blanks of {kind} removed"))
    return found


EXAMPLES = {}
