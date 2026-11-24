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
        self.src, self.string = src, cfg["string_kind"]
        self.kind, self.start, self.end = [], [], []
        self.sb, self.eb, self.br, self.dr, self.cm, self.frame = [], [], [], [], [], []
        stack = []  # frames: [name, tags, first, first_code, anchor, boundary]
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
                if "directive" in f[1]:
                    depth_dir -= 1
                if conts and conts[-1] is f:
                    conts.pop()
                nfirst, ncode = min(nfirst, len(stack)), min(ncode, len(stack))
            name, rng, *rest = body.split(" ", 2)
            s, e = map(int, rng.split(".."))
            if not rest:  # a node
                tg = tags.get(name, set())
                parent = stack[-1] if stack else None
                f = [name, tg, None, None, None, bool(tg & bound) and not (parent and parent[0] in inside)]
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
            self.br.append(name in braces and bool(stack) and "block" in stack[-1][1])
            self.dr.append(depth_dir > 0)
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


def tokens_equal(a, b):
    """None when equal; else the kind of difference."""
    ta, tb = [norm(a, i) for i in range(a.n)], [norm(b, i) for i in range(b.n)]
    if ta == tb:
        return None
    if sorted(ta) == sorted(tb):
        return "tokens-reorder"

    def merge(tr, toks):
        out, prev = [], False
        for t, k in zip(toks, tr.kind):
            q = k == tr.string and t.startswith(b'"')
            if q and prev:
                out[-1] = out[-1][:-1] + t[1:]
            else:
                out.append(t)
            prev = q
        return out

    if merge(a, ta) == merge(b, tb):
        return "tokens-string-split"
    k = next((i for i, (x, y) in enumerate(zip(ta, tb)) if x != y), min(len(ta), len(tb)))
    return "tokens-comment" if any(t.cm[k] for t in (a, b) if k < t.n) else "tokens-other"


def compare(ref, out, shape=None, where=None):
    """(equal, causes in file order, wrap gaps that differ in newline) of two Trees; the tree facts (wrap points,
    anchors) are those of `shape`, by default the output's."""
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
        if not nr and not no:
            if gr != go:
                note("comment-gap" if shape.cm[i] or (i and shape.cm[i - 1]) else "spacing")
            continue
        cm = shape.cm[i] or (i and shape.cm[i - 1])
        brace = shape.br[i] or (i and shape.br[i - 1])
        exact = i == 0 or i == out.n - 1 or shape.sb[i] or (i and shape.eb[i - 1]) or brace or cm
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

    def score(self, theirs, ours, suffix, ref_tree=None):
        """The result of one file: ok, causes, wraps; the column-limit guard is added by the caller. `ours` is a text,
        or a Tree built from the reference's own tokens (a control), whose wrap points are the reference's."""
        ref = ref_tree or self.tree(theirs, suffix)
        out = ours if isinstance(ours, Tree) else self.tree(ours, suffix)
        if ref is None or out is None:
            return {"ok": False, "causes": ["tree"], "wraps": 0}
        ok, causes, wraps = compare(ref, out, ref if out is ours else None)
        return {"ok": ok, "causes": causes, "wraps": wraps}


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
        if not (ref.sb[i] or ref.eb[i - 1] or ref.br[i] or ref.br[i - 1] or ref.cm[i] or ref.cm[i - 1]):
            wrap.append(i)
    wrapset = set(wrap)

    base = [0]  # the indent of the line that began the current statement

    def rebuild(pick):
        """The reference text with its wrap gaps replaced by `pick(i, gap)`; the lines of a block that is anchored to a
        moved line (a lambda body, an initializer) move with it, as a formatter would indent them."""
        out, cur, line = [], [], 0
        base[0] = 0
        starts, ends, pos = [], [], 0
        for i in range(ref.n):
            g = ref.gap(i)
            if i in wrapset:
                g = pick(i, g, line)
            elif g.count(b"\n"):
                a = ref.anchor(i)
                delta = cur[a] - ref.indent(a) if 0 <= a < len(cur) else 0
                if delta:
                    head, last = g.rsplit(b"\n", 1)
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
        return view

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

    lines = theirs.split("\n")
    blanks = [k for k, l in enumerate(lines) if not l.strip()]
    drop = set(k for k in blanks if rnd.random() < 0.05)
    return {
        "join-all": rebuild(join),
        "split-all": rebuild(split),
        "random": rebuild(rand),
        "drop-blank-5pct": "\n".join(l for k, l in enumerate(lines) if k not in drop),
    }


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

    def one(text):
        if text is None:
            return {"ok": False, "causes": ["rejected"], "wraps": 0, "byte": False, "guard": False}
        r = checker.score(theirs, text, suffix, ref)
        raw = text.src.decode("utf-8") if isinstance(text, Tree) else text
        r.update(byte=raw == theirs, guard=over_limit(raw, limit) > base)
        return r

    rec = one(ours)
    keep = os.environ.get("PARITY_KEEP")
    if keep:
        d = Path(keep) / copy.parent.name / copy.name
        d.parent.mkdir(parents=True, exist_ok=True)
        (d.parent / (d.name + ".R")).write_text(theirs, encoding="utf-8")
        if ours is not None:
            (d.parent / (d.name + ".S")).write_text(ours, encoding="utf-8")
    if controlled and ref is not None:
        c = {"verbatim": one(copy.read_text(encoding="utf-8"))}
        for name, text in controls(checker, ref, theirs, suffix, copy.name).items():
            c[name] = one(text)
        for name in cfg["variants"]:
            c[f"style-{name}"] = one(variant(name))
        rec["controls"] = c
    return rec


def first_only(causes):
    return (causes[0], causes[0] if len(set(causes)) == 1 else None) if causes else (None, None)


def report(found, results, wrapped, target, label):
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
