"""Token-and-gap comparison of a formatter output with the reference output (`wrap_compare` of an entry of parity_tools.json).

The reference tool picks line breaks by penalties; Sudsy's printer does not. A file is equal when the tokens of both outputs
are equal in order (comments are tokens) and every gap (the whitespace between two tokens) is equal, except at a WRAP POINT:
a gap inside one statement, declaration or directive that is not next to a block brace, not next to a comment and not a
boundary between two statements or members. At a wrap point a newline equals a space or nothing, and the indentation after a
newline is ignored; when neither side has a newline the gap must be equal. Wrap points come from Sudsy's own syntax tree of
the output (`debug tree`): the node kinds and tags of the language's grammar file, named in the entry. Every other gap is
compared exactly: newline count, brace placement, spacing, and the indentation of the first token of a line relative to the
line that anchors its enclosing block (the line where the statement that owns the block starts, where the lambda starts,
or the line of an initializer's `{`; so the indentation of a wrapped enclosing statement does not matter).
Adjacent string literals that the reference tool split (BreakStringLiterals) are one literal: see `string_joins`.
"""
import copy
import os
import random
import re
import subprocess
import tempfile
import zlib
from pathlib import Path

TAB = 8
BOM = b"\xef\xbb\xbf"


def grammar_tags(path):
    """kind -> set of tags of the `node` lines of a grammar file."""
    tags = {}
    for line in Path(path).read_text(encoding="utf-8").splitlines():
        m = re.match(r"node (\w+)(.*?)(?: =|$)", line)
        if m:
            tags[m.group(1)] = set(re.findall(r"#([\w-]+)", m.group(2)))
    return tags


class Tree:
    """The real tokens of a source with their tree facts."""

    def __init__(self, cfg, tags, dump, src):
        gap, com, eof, braces = set(cfg["gap_kinds"]), cfg["comment_kind"], cfg["eof_kind"], set(cfg["brace_kinds"])
        bound, cont_tags, inside = set(cfg["boundary"]), set(cfg["container"]), set(cfg["wrap_inside"])
        if src.startswith(BOM):  # the dump is of the text without its byte order mark
            src = src[len(BOM):]
        self.src, self.string = src, cfg["string_kind"]
        self.kind, self.start, self.end = [], [], []
        self.sb, self.eb, self.br, self.dr, self.cm, self.frame = [], [], [], [], [], []
        self.par, self.gpar = [], []  # the kinds of the parent node and of its parent
        self.sh = []  # the gap before the token is inside a short body: its `ex` facts (see `short_body`), or None
        self.mb, self.off = [], []  # the token ends a statement of a macro body; the token is between `format_off` and `format_on`
        mkinds, pars, off, pdepth = set(cfg["macro_boundary"]), set(cfg["paren_kinds"]), False, 0
        short = cfg.get("short_bodies") or {}
        stack = []  # frames: [name, tags, first, first_code, anchor, boundary, ex]; ex: the facts of a short-body block
        nfirst = ncode = 0
        depth_dir = 0
        conts = []
        for line in dump.split("\n"):
            if not line:
                continue
            body = line.lstrip(" ")
            depth = (len(line) - len(body)) // 2
            while len(stack) > depth:
                f = stack.pop()
                if f[5] and f[2] is not None:
                    self.eb[len(self.kind) - 1] = True
                ex = f[6]
                if ex and "opt" in ex and ex["n"] == 1 and not ex["cm"] and not ex["nb"] and ex["lb"] is not None and ex["rb"] is not None:
                    self.sh[ex["lb"] + 1] = self.sh[ex["rb"]] = ex
                if "directive" in f[1]:
                    depth_dir -= 1
                    pdepth = 0
                if conts and conts[-1] is f:
                    conts.pop()
                nfirst, ncode = min(nfirst, len(stack)), min(ncode, len(stack))
            name, rng, *rest = body.split(" ", 2)
            s, e = map(int, rng.split(".."))
            if not rest:  # a node
                tg = tags.get(name, set())
                parent = stack[-1] if stack else None
                f = [name, tg, None, None, None, bool(tg & bound) and not (parent and parent[0] in inside), None]
                if parent and parent[6] and "opt" in parent[6]:
                    parent[6]["n"] += 1
                if "block" in tg:
                    for a in stack:
                        if a[6] and "opt" in a[6]:
                            a[6]["nb"] = True
                if short and "block" in tg and parent:
                    opt = short["owners"].get(parent[0]) or (short["nested"] if parent[0] == name else None)
                    if opt:
                        up = stack[-2][0] if len(stack) > 1 else ""
                        f[6] = {"opt": opt, "n": 0, "cm": False, "nb": False, "lb": None, "rb": None, "owner": parent,
                                "inline": parent[0] in short["owners"] and up == name and len(stack) > 2 and stack[-3][0] in short["class_kinds"],
                                "arg": up in short["argument_kinds"]}
                if tg & cont_tags:
                    f[4] = -1 if parent is None else "self" if (tg & {"huggable"}) or (parent[1] & cont_tags) else parent
                    conts.append(f)
                if "directive" in tg:
                    depth_dir += 1
                stack.append(f)
                continue
            if name in gap:
                continue
            t = len(self.kind)
            is_com = name == com
            self.kind.append(name), self.start.append(s), self.end.append(e)
            self.sh.append(None)
            for a in stack:
                if is_com and a[6] and "opt" in a[6]:
                    a[6]["cm"] = True
            if stack and stack[-1][6] and "opt" in stack[-1][6] and name in braces:
                stack[-1][6]["lb" if name == cfg["brace_kinds"][0] else "rb"] = t
            if stack and stack[-1][0] == "IfStatement" and src[s:e] == b"else":
                stack[-1][6] = {"else": True}
            self.sb.append(False), self.eb.append(False)
            while nfirst < len(stack):
                f = stack[nfirst]
                f[2] = t
                if f[5]:
                    self.sb[t] = True
                nfirst += 1
            if not is_com:
                while ncode < len(stack):
                    f = stack[ncode]
                    f[3] = t
                    if f[4] == "self":
                        f[4] = t
                    ncode += 1
            self.par.append(stack[-1][0] if stack else ""), self.gpar.append(stack[-2][0] if len(stack) > 1 else "")
            self.br.append(name in braces and bool(stack) and "block" in stack[-1][1])
            self.dr.append(depth_dir > 0)
            pdepth += (name in pars) * (1 if name == cfg["paren_kinds"][0] else -1)
            self.mb.append(depth_dir > 0 and name in mkinds and pdepth <= 0)
            text = src[s:e]
            if is_com and cfg["format_on"].encode() in text:
                off = False
            self.off.append(off)
            if is_com and cfg["format_off"].encode() in text:
                off = True
            self.cm.append(is_com or name == eof)
            self.frame.append(conts[-1] if conts else None)
            if name == eof:
                self.cm[t] = True
        for f in reversed(stack):  # the frames the dump ends in
            if f[5] and f[2] is not None:
                self.eb[len(self.kind) - 1] = True
        self.n = len(self.kind)

    def anchor(self, t):
        """The token whose line anchors the indentation of token t, or -1."""
        f = self.frame[t]
        if f is None:
            return -1
        a = f[4]
        while isinstance(a, list):  # the owner of the block: its first token of code
            a = a[3] if a[3] is not None else f[3]
        return a if isinstance(a, int) else -1

    def indent(self, t):
        if t < 0:
            return 0
        ls = self.src.rfind(b"\n", 0, self.start[t]) + 1
        line = self.src[ls : self.start[t]]
        return width(line[: len(line) - len(line.lstrip(b" \t"))])

    def gap(self, i):
        return self.src[(self.end[i - 1] if i else 0) : self.start[i]]


def width(ws):
    col = 0
    for c in ws:
        col = (col // TAB + 1) * TAB if c == 9 else col + 1
    return col


def norm(tree, i):
    t = tree.src[tree.start[i] : tree.end[i]]
    return re.sub(rb"[ \t]*\n[ \t]*", b"\n", t) if tree.cm[i] else t


STRING = re.compile(rb'^(u8|u|U|L)?"(.*)"$', re.S)


def string_runs(tree):
    """The runs of adjacent string literals with one prefix (a raw string has none; a directive and the text under
    `clang-format off` have no runs): [first token, one past the last, prefix, joined contents]. A single literal is a run."""
    runs = []
    for i in range(tree.n):
        m = None
        if tree.kind[i] == tree.string and not tree.dr[i] and not tree.off[i]:
            m = STRING.match(tree.src[tree.start[i] : tree.end[i]])
        if not m:
            continue
        prefix = m.group(1) or b""
        if runs and runs[-1][1] == i and runs[-1][2] == prefix:
            runs[-1][1] = i + 1
            runs[-1][3] += m.group(2)
        else:
            runs.append([i, i + 1, prefix, m.group(2)])
    return runs


def string_joins(ref, out, src):
    """BreakStringLiterals: the runs of `ref` and of `out` to join into one literal, or None when a difference is not the
    tool's split. The runs are equal in order, prefix and joined contents; where their literal counts differ the tool must
    have joined or split the run, so its count is not the one of the source tree `src` (a source of two literals that Sudsy
    changed is a difference)."""
    rr, ro, rs = string_runs(ref), string_runs(out), string_runs(src)
    if len(rr) != len(ro) or any(a[2:] != b[2:] for a, b in zip(rr, ro)):
        return None
    joins = ([], [])
    for k, (a, b) in enumerate(zip(rr, ro)):
        na, nb = a[1] - a[0], b[1] - b[0]
        if na == nb:
            continue
        if len(rs) != len(rr) or na == rs[k][1] - rs[k][0]:
            return None
        for side, run, n in ((0, a, na), (1, b, nb)):
            if n > 1:
                joins[side].append(run)
    return joins


def joined_text(tree, runs):
    """The text of `tree` with each of `runs` written as one literal."""
    text, pos = [], 0
    for first, last, prefix, body in runs:
        text += [tree.src[pos : tree.start[first]], prefix + b'"' + body + b'"']
        pos = tree.end[last - 1]
    return b"".join(text + [tree.src[pos:]]).decode("utf-8")


def tokens_equal(a, b):
    """None when equal; else the kind of difference."""
    ta, tb = [norm(a, i) for i in range(a.n)], [norm(b, i) for i in range(b.n)]
    if ta == tb:
        return None
    if sorted(ta) == sorted(tb):
        return "tokens-reorder"

    def merge(tr, toks):
        out, i, runs = [], 0, {r[0]: r for r in string_runs(tr)}
        while i < len(toks):
            r = runs.get(i)
            out.append(r[2] + b'"' + r[3] + b'"' if r else toks[i])
            i = r[1] if r else i + 1
        return out

    if merge(a, ta) == merge(b, tb):
        return "tokens-string-split"
    k = next((i for i, (x, y) in enumerate(zip(ta, tb)) if x != y), min(len(ta), len(tb)))
    return "tokens-comment" if any(t.cm[k] for t in (a, b) if k < t.n) else "tokens-other"


def joinable(ex, allow, table):
    """The body of a short-body block (the `ex` facts of `Tree.sh`) may be joined to one line under the style's
    AllowShort...OnASingleLine values `allow`: `table` maps option -> value -> the fact the construct needs (`inline`,
    `arg`, `!else`; empty for none); an option the style sets to None/Never/false, or a value not in the table, forces the form."""
    need = table.get(ex["opt"], {}).get(str(allow.get(ex["opt"])))
    if need is None:
        return False
    if need == "!else":
        return not (ex["owner"][6] and ex["owner"][6].get("else"))
    return not need or bool(ex[need])


def compare(ref, out, shape=None, where=None, joins=None):
    """(equal, causes in file order, wrap gaps that differ in newline) of two Trees; the tree facts (wrap points,
    anchors) are those of `shape`, by default the output's. `joins`: a function ex -> bool; the newlines inside a short body
    that it accepts are wrap points: the width alone decides them, and the lines after them are free as after any wrap."""
    shape = shape or out
    why = tokens_equal(ref, out)
    if why:
        return False, [why], 0
    causes, wraps = [], 0

    def note(c):
        causes.append(c)
        if where is not None:
            where.append(i)

    for i in range(out.n):
        gr, go = ref.gap(i), out.gap(i)
        nr, no = gr.count(b"\n"), go.count(b"\n")
        if shape.off[i] or (i and shape.off[i - 1]):  # under `clang-format off` both tools keep the text
            if [l.rstrip() for l in gr.split(b"\n")] != [l.rstrip() for l in go.split(b"\n")]:
                note("format-off")
            continue
        if not nr and not no:
            if gr != go:
                note("comment-gap" if shape.cm[i] or (i and shape.cm[i - 1]) else "spacing")
            continue
        cm = shape.cm[i] or (i and shape.cm[i - 1])
        brace = shape.br[i] or (i and shape.br[i - 1])
        exact = i == 0 or i == out.n - 1 or shape.sb[i] or (i and (shape.eb[i - 1] or shape.mb[i - 1])) or brace or cm
        if exact and joins and shape.sh[i] and not cm and i != out.n - 1 and joins(shape.sh[i]):
            exact = False
        if not exact:
            if nr and no:
                ok = min(nr, 2) == min(no, 2)
            else:
                plain, nl = (go, nr) if nr else (gr, no)
                ok = not plain.strip(b" \t") and nl == 1
            wraps += (nr > 0) != (no > 0)
            if not ok:
                note("wrap-blank")
            continue
        if (nr > 0) != (no > 0):
            note("comment-gap" if cm else "brace-placement" if brace else "statement-join")
        elif nr != no:
            note("comment-gap" if cm else "blank-lines")
        else:
            hr, ho = [s.rstrip() if b"\\" not in s else s for s in gr.split(b"\n")[:-1]], [s.rstrip() if b"\\" not in s else s for s in go.split(b"\n")[:-1]]
            relr = width(gr.rsplit(b"\n", 1)[1]) - ref.indent(shape.anchor(i))
            relo = width(go.rsplit(b"\n", 1)[1]) - out.indent(shape.anchor(i))
            if hr != ho or relr != relo:
                note("comment-gap" if cm else "indent")
    return not causes, causes, wraps


def gap_text(g):
    """A gap as the gap list writes it: its blanks (`sp` for a long run), or its line breaks."""
    if b"\n" in g:
        return "nl" + str(g.count(b"\n"))
    return g.decode("latin-1").replace("\t", "\\t") if len(g) <= 3 else f"{len(g)}sp"


def gaps(ref, out, joins=None):
    """Every difference of two Trees as records {c: cause, a, b: kinds of the tokens around the gap, p, g: parent and
    grandparent kinds of the second token, o, t: the gap of `out` and of the reference, x: the two token texts}; when the
    tokens differ, the first pair that does (`parity_gaps.py` ranks them)."""
    def text(tree, i):
        return tree.src[tree.start[i] : tree.end[i]][:40].decode("utf-8", "replace")

    ta, tb = [norm(ref, i) for i in range(ref.n)], [norm(out, i) for i in range(out.n)]
    if ta != tb:
        k = next((i for i, (x, y) in enumerate(zip(ta, tb)) if x != y), min(len(ta), len(tb)))
        if k < min(ref.n, out.n):
            return [{"c": "tokens-reorder" if sorted(ta) == sorted(tb) else "tokens", "a": ref.kind[k], "b": out.kind[k], "p": out.par[k], "g": out.gpar[k], "o": text(out, k), "t": text(ref, k), "x": text(out, k - 1) if k else ""}]
        return [{"c": "tokens", "a": "", "b": "", "p": "", "g": "", "o": f"{out.n} tokens", "t": f"{ref.n} tokens", "x": ""}]
    where = []
    _, causes, _ = compare(ref, out, None, where, joins)

    def wrapped_before(i):
        """A wrap point between the anchor of token i (else the start of its statement) and i has a newline in one output only."""
        s = out.anchor(i)
        if s < 0:
            s = next((j for j in range(i, -1, -1) if out.sb[j]), 0)
        for k in range(s + 1, i):
            exact = out.sb[k] or out.eb[k - 1] or out.mb[k - 1] or out.br[k] or out.br[k - 1] or out.cm[k] or out.cm[k - 1]
            if not exact and (b"\n" in ref.gap(k)) != (b"\n" in out.gap(k)):
                return 1
        return 0

    return [
        {"c": c, "a": out.kind[i - 1] if i else "", "b": out.kind[i], "p": out.par[i], "g": out.gpar[i], "o": gap_text(out.gap(i)), "t": gap_text(ref.gap(i)), "x": (text(out, i - 1) if i else "") + " " + text(out, i),
         "w": wrapped_before(i) if c in ("indent", "comment-gap", "brace-placement") else 0}
        for c, i in list(zip(causes, where))[:20000]
    ]


class Checker:
    """Parses texts with the dump command of Sudsy and compares them."""

    def __init__(self, cfg, root, dump_command, tmp):
        self.cfg, self.cmd, self.tmp = cfg, dump_command, Path(tmp)
        self.tmp.mkdir(parents=True, exist_ok=True)
        self.tags = grammar_tags(Path(root) / cfg["grammar"])

    def tree(self, text, suffix):
        data = text.encode("utf-8")
        with tempfile.NamedTemporaryFile(dir=self.tmp, suffix=suffix, delete=False) as f:
            f.write(data)
        try:
            r = subprocess.run([*self.cmd, f.name], capture_output=True)
        finally:
            Path(f.name).unlink()
        if r.returncode != 0:
            return None
        return Tree(self.cfg, self.tags, r.stdout.decode("utf-8", "replace"), data)

    def score(self, theirs, ours, suffix, ref_tree=None, source=None, listing=False, join_ok=None):
        """The result of one file: ok, causes, wraps; the column-limit guard is added by the caller. `ours` is a text,
        or a Tree built from the reference's own tokens (a control), whose wrap points are the reference's. `source` is the
        text both tools read: a string literal the reference tool split is compared as the whole literal."""
        ref = ref_tree or self.tree(theirs, suffix)
        out = ours if isinstance(ours, Tree) else self.tree(ours, suffix)
        if ref is None or out is None:
            return {"ok": False, "causes": ["tree"], "wraps": 0}
        ok, causes, wraps = compare(ref, out, ref if out is ours else None, None, join_ok)
        joins = string_joins(ref, out, self.tree(source, suffix)) if causes == ["tokens-string-split"] and source is not None else None
        if joins:
            ref, out = (self.tree(joined_text(t, j), suffix) if j else t for t, j in zip((ref, out), joins))
            if ref is None or out is None:
                return {"ok": False, "causes": ["tree"], "wraps": 0}
            ok, causes, wraps = compare(ref, out, None, None, join_ok)
        r = {"ok": ok, "causes": causes, "wraps": wraps}
        if listing and not ok:
            r["gaps"] = gaps(ref, out, join_ok)
        return r


def over_limit(text, limit, tab=TAB):
    if not limit:
        return 0
    n = 0
    for line in text.split("\n"):
        if len(line) > limit // 2 and len(line.expandtabs(tab)) > limit:
            n += 1
    return n


def controls(chk, ref, theirs, suffix, name):
    """The damaged copies of the reference output that need only its tree: name -> text."""
    rnd = random.Random(zlib.crc32(name.encode()))
    src = ref.src
    wrap = []  # (gap index) of wrap points of the reference tree
    for i in range(1, ref.n - 1):
        if not (ref.sb[i] or ref.eb[i - 1] or ref.mb[i - 1] or ref.br[i] or ref.br[i - 1] or ref.cm[i] or ref.cm[i - 1] or ref.off[i] or ref.off[i - 1] or ref.gap(i).count(b"\n") > 1):
            wrap.append(i)
    wrapset = set(wrap)

    base = [0]  # the indent of the line that began the current statement

    def rebuild(pick):
        """The reference text with its wrap gaps replaced by `pick(i, gap)`; the lines of a block that is anchored to a
        moved line (a lambda body, an initializer) move with it, as a formatter would indent them. When a block would
        have to move left of column 0 (its anchor line was joined to a less indented one), the wrap gaps of the anchor's
        line stay as they are: no output can have that block at its reference offset from the anchor."""
        pinned, state = set(), rnd.getstate()
        while True:
            rnd.setstate(state)
            view, clamped = rebuild_once(pick, pinned)
            if not clamped:
                return view
            before = len(pinned)
            for a in clamped:
                s = a
                while s and b"\n" not in ref.gap(s):
                    s -= 1
                pinned.update(j for j in range(s, a + 1) if j in wrapset)
            if len(pinned) == before:
                return view

    def rebuild_once(pick, pinned):
        out, cur, line, clamped = [], [], 0, set()
        base[0] = 0
        starts, ends, pos = [], [], 0
        for i in range(ref.n):
            g = ref.gap(i)
            if i in wrapset and i not in pinned:
                g = pick(i, g, line)
            elif g.count(b"\n"):
                a = ref.anchor(i)
                delta = cur[a] - ref.indent(a) if 0 <= a < len(cur) else 0
                if delta:
                    head, last = g.rsplit(b"\n", 1)
                    if width(last) + delta < 0:
                        clamped.add(a)
                    g = head + b"\n" + b" " * max(0, width(last) + delta)
            if b"\n" in g:
                line = width(g.rsplit(b"\n", 1)[1])
                if i not in wrapset:
                    base[0] = line
            cur.append(line)
            out.append(g)
            tok = src[ref.start[i] : ref.end[i]]
            out.append(tok)
            starts.append(pos + len(g))
            pos += len(g) + len(tok)
            ends.append(pos)
        view = copy.copy(ref)
        view.src, view.start, view.end = b"".join(out), starts, ends
        return view, clamped

    def brk(i, indent):
        return (b" \\\n" if ref.dr[i] else b"\n") + b" " * indent

    def join(i, g, line):  # `line`: the indent of the current line
        return b" " if b"\n" in g else g

    def split(i, g, line):
        return g if b"\n" in g else brk(i, base[0] + 4)

    def rand(i, g, line):
        c = rnd.random()
        if c < 0.34:
            return join(i, g, line)
        return g if c < 0.67 else brk(i, rnd.randrange(0, 40))

    def strings(damage):
        """The reference text with the contents of its first plain string literal (outside a directive and
        `clang-format off`) damaged: one byte changed, or the literal split in two. The text is unchanged without one."""
        for i in range(ref.n):
            m = STRING.match(src[ref.start[i] : ref.end[i]]) if ref.kind[i] == ref.string and not ref.dr[i] and not ref.off[i] else None
            if m and len(m.group(2)) > 1 and b"\\" not in m.group(2) and m.group(2).isascii():
                a, b = ref.start[i] + len(m.group(1) or b"") + 1, ref.end[i] - 1
                mid = (a + b) // 2
                if damage:
                    new = src[:a] + (b"Y" if src[a : a + 1] == b"X" else b"X") + src[a + 1 :]
                else:
                    new = src[:mid] + b'" "' + src[mid:]
                return new.decode("utf-8")
        return theirs

    lines = theirs.split("\n")
    blanks = [k for k, l in enumerate(lines) if not l.strip()]
    drop = set(k for k in blanks if rnd.random() < 0.05)
    return {
        "join-all": rebuild(join),
        "split-all": rebuild(split),
        "random": rebuild(rand),
        "damage-string": strings(True),
        "split-string": strings(False),
        "drop-blank-5pct": "\n".join(l for k, l in enumerate(lines) if k not in drop),
    }


def style_values(cfg, folder, style):
    """The AllowShort...OnASingleLine values of the style's file: its own, else those of its BasedOnStyle."""
    text = (Path(folder) / f"{style}.clang-format").read_text(encoding="utf-8")
    base = re.search(r"^BasedOnStyle:\s*(\w+)", text, re.M)
    values = dict(cfg["short_bodies"]["defaults"].get(base.group(1) if base else "LLVM", {}))
    values.update(re.findall(r"^(AllowShort\w+OnASingleLine):\s*(\w+)", text, re.M))
    return values


def column_limit(cfg, folder, style):
    """The ColumnLimit of the style's file: its own value or its BasedOnStyle's."""
    text = (Path(folder) / f"{style}.clang-format").read_text(encoding="utf-8")
    m = re.search(r"^ColumnLimit:\s*(\d+)", text, re.M)
    base = re.search(r"^BasedOnStyle:\s*(\w+)", text, re.M)
    return int(m.group(1)) if m else cfg["column_limits"].get(base.group(1) if base else "LLVM", 80)


def write_variants(cfg, folder, out, styles):
    """`<style>~<variant>.clang-format` for each variant of the entry: the style file with one option changed (the
    controls that must score low: another brace or pointer-alignment style)."""
    out.mkdir(parents=True, exist_ok=True)
    for style in styles:
        text = (Path(folder) / f"{style}.clang-format").read_text(encoding="utf-8")
        base = (re.search(r"^BasedOnStyle:\s*(\w+)", text, re.M) or [0, "LLVM"])[1]
        for name, v in cfg["variants"].items():
            key = v["key"]
            kept = [l for l in text.splitlines() if not l.startswith(f"{key}:")]
            kept.append(f"{key}: {v['values'][1] if base in v['alt_for'] else v['values'][0]}")
            kept += [f"{k}: {x}" for k, x in v.get("extra", {}).items()]
            (out / f"{style}~{name}.clang-format").write_text("\n".join(kept) + "\n", encoding="utf-8")


def file_result(checker, cfg, folder, style, theirs, ours, copy, controlled, variant):
    """The result of one file: byte-identical, equal under the comparison, causes, guard, and with `controlled` the
    same for each control."""
    suffix = copy.suffix
    limit = column_limit(cfg, folder, style)
    ref = checker.tree(theirs, suffix)
    base = over_limit(theirs, limit)
    short = cfg.get("short_bodies")
    allow = style_values(cfg, folder, style) if short else {}
    joins = (lambda ex: joinable(ex, allow, short["permits"])) if short else None
    source = copy.read_text(encoding="utf-8")

    def one(text, listing=False):
        if text is None:
            return {"ok": False, "causes": ["rejected"], "wraps": 0, "byte": False, "guard": False}
        r = checker.score(theirs, text, suffix, ref, source, listing, joins)
        raw = text.src.decode("utf-8") if isinstance(text, Tree) else text
        r.update(byte=raw == theirs, guard=over_limit(raw, limit) > base)
        return r

    rec = one(ours, bool(os.environ.get("PARITY_GAPS")))
    keep = os.environ.get("PARITY_KEEP")
    if keep:
        d = Path(keep) / copy.parent.name / copy.name
        d.parent.mkdir(parents=True, exist_ok=True)
        (d.parent / (d.name + ".R")).write_text(theirs, encoding="utf-8")
        if ours is not None:
            (d.parent / (d.name + ".S")).write_text(ours, encoding="utf-8")
    if controlled and ref is not None:
        c = {"self": one(theirs), "verbatim": one(source)}
        for name, text in controls(checker, ref, theirs, suffix, copy.name).items():
            c[name] = one(text)
        for name in cfg["variants"]:
            c[f"style-{name}"] = one(variant(name))
        rec["controls"] = c
    return rec


def first_only(causes):
    return (causes[0], causes[0] if len(set(causes)) == 1 else None) if causes else (None, None)


def report(found, results, wrapped, target, label, note=""):
    """Per group: files, rejected, byte-identical, equal, guard, second format, refused; the causes; the controls."""
    import json
    from collections import Counter

    rows, causes, only, ctl = {}, Counter(), Counter(), {}
    group_of = {n: g for (n, _, g, _) in found}
    for (name, _, group, _), (theirs, ours, unstable, refused) in zip(found, results):
        g = rows.setdefault(group or "fixtures", Counter())
        g["files"] += 1
        w = wrapped.get(name)
        if w is None:
            g["rejected"] += 1
            continue
        g["byte"] += w["byte"]
        g["equal"] += w["ok"]
        g["guard"] += w["guard"]
        g["wraps"] += w["wraps"] > 0
        g["second"] += bool(unstable)
        g["refused"] += bool(refused)
        if not w["ok"]:
            f, o = first_only(w["causes"])
            causes[f] += 1
            if o:
                only[o] += 1
        for k, c in w.get("controls", {}).items():
            t = ctl.setdefault(k, Counter())
            t["files"] += 1
            t["equal"] += c["ok"]
            t["byte"] += c["byte"]
            t["guard"] += c["guard"]
    print(note)
    print(f"== wrap compare [{label}]: group files rejected byte equal guard second refused (equal files whose wraps differ)")
    tot = Counter()
    for g, c in sorted(rows.items()):
        tot.update(c)
        print(f"{g:10} {c['files']:5} {c['rejected']:4} {c['byte']:5} {c['equal']:5} {c['guard']:5} {c['second']:3} {c['refused']:3} ({c['wraps']})")
    print(f"{'total':10} {tot['files']:5} {tot['rejected']:4} {tot['byte']:5} {tot['equal']:5} {tot['guard']:5} {tot['second']:3} {tot['refused']:3} ({tot['wraps']})")
    print("first cause:", ", ".join(f"{k} {v}" for k, v in causes.most_common()))
    print("only cause:", ", ".join(f"{k} {v}" for k, v in only.most_common()))
    for k, t in ctl.items():
        print(f"control {k}: {t['files']} files, equal {t['equal']}, byte {t['byte']}, guard {t['guard']}")
    out = Path(target).resolve() / "parity" / f"wrap-{label}.json"
    out.write_text(json.dumps({"rows": {g: dict(c) for g, c in rows.items()}, "files": {n: {**w, "group": group_of[n]} for n, w in wrapped.items()}}), encoding="utf-8")
