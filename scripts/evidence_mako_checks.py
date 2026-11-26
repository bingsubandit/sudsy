"""Lint checklist of the Mako evidence (docs/ai/design/mako.md): Mako's own compiler as the validity reference, and the yield of
each checklist entry on the corpus (occurrences, files, projects, held-out files).

Usage: python3 -I scripts/evidence_mako_checks.py   (needs `mako` installed: pip install mako==1.4.3)

The validity reference is Mako's lexer and code generator (what `Template` runs first, mako/template.py `_compile`); the
generated module is compiled by Python but not executed, so a `<%! import x %>` of a module the checker does not have is not
a failure. Each entry names its source (Mako 1.4.3 `lexer.py`, `pyparser.py`, `codegen.py`, `parsetree.py` messages; the
Mako docs `doc/build/syntax.rst`, `defs.rst`, `filtering.rst` at sqlalchemy/mako 14889bc0). The compiler stops at the first
error of a file, so the compile entries count one error per file. The regex entries count every occurrence.
"""
import re
import sys
import warnings
from collections import defaultdict
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import evidence  # noqa: E402
import evidence_mako as mod  # noqa: E402
from mako import codegen, exceptions  # noqa: E402
from mako.lexer import Lexer  # noqa: E402

warnings.simplefilter("ignore")

# (checklist entry, source, the Mako message that shows it; "" for a Python SyntaxError of the generated module).
COMPILE = [
    ("control block not closed at the end of the file", "lexer.py:278; syntax.rst 'closed by % end'", "Unterminated control keyword"),
    ("end keyword with no open block", "lexer.py:471", "No starting keyword"),
    ("end keyword does not match its block", "lexer.py:476", "doesn't match keyword"),
    ("control line with no keyword", "lexer.py:462", "Invalid control line"),
    ("tag not closed (<%def>, <%block>, <%text>, ...)", "lexer.py:273, 330; defs.rst", "Unclosed tag"),
    ("closing tag with no open tag, or of another tag", "lexer.py:342, 348", "Closing tag"),
    ("expression or <% %> block not terminated", "lexer.py:126", "unterminated tag or expression"),
    ("unknown tag name", "parsetree.py:241", "No such tag"),
    ("Python syntax error in an expression or a block", "pyparser.py:43 '(SyntaxError)'; Python compile of the module", "(SyntaxError)"),
    ("two defs or blocks with one name", "codegen.py:1165", "already exists in this template"),
    ("import * in a Python block", "pyparser.py:197", "'import *' is not supported"),
    ("namespace tag with neither name nor import", "parsetree.py:415", "'name' and/or 'import' attributes are required"),
]
REGEX = [
    ("self.NAME() call with no def NAME in the file (no inherit)", "defs.rst: `self` is the template's own namespace, its defs and inherited ones"),
    ("expression with the n filter (default escaping off)", "filtering.rst: `n` disables the default filters"),
]
SELF_CALL = re.compile(r"\$\{\s*(?:self|parent)\.(\w+)\s*\(")
DEF = re.compile(r"<%\s*def\s+name\s*=\s*[\"'](\w+)")
INHERIT = re.compile(r"<%\s*inherit\b")


def compile_error(text):
    """The message of the first error of the template (Mako's lexer and code generator, then Python's compile), or None."""
    try:
        lexer = Lexer(text, "template.mako", input_encoding=None)
        node = lexer.parse()
        src = codegen.compile(
            node, "template.mako", "template.mako", default_filters=["str"], buffer_filters=[], imports=None,
            future_imports=None, source_encoding=lexer.encoding, generate_magic_comment=False, strict_undefined=False,
            enable_loop=True, reserved_names=frozenset(),
        )
    except Exception as e:  # noqa: BLE001 - Mako raises its own classes for syntax and compile errors
        return str(e)
    try:
        compile(src, "template.mako", "exec")
    except SyntaxError as e:
        return f"(SyntaxError) generated module: {e.msg}"
    return None


def classify(msg):
    for entry, _, needle in COMPILE:
        if needle in msg:
            return entry
    return "other: " + msg.splitlines()[0][:100]


def main():
    corpus = evidence.files(mod)
    _, held = evidence.split(corpus, mod.SEED)
    rows = []
    for proj, name, f in corpus:
        text = evidence.read(f)
        if text is not None:
            rows.append((proj, name, text, proj in held))
    errors = {name: compile_error(t) for _, name, t, _ in rows}
    print(f"Mako accepts {sum(e is None for e in errors.values())} of {len(rows)} files ({len({p for p, _, _, _ in rows})} projects)\n")

    hits = defaultdict(lambda: [0, set(), set(), 0])  # occurrences, files, projects, held-out files

    def add(entry, proj, name, n, is_held):
        hits[entry][0] += n
        hits[entry][1].add(name)
        hits[entry][2].add(proj)
        hits[entry][3] += is_held

    other = defaultdict(int)
    for proj, name, text, is_held in rows:
        msg = errors[name]
        if msg is not None:
            entry = classify(msg)
            if entry.startswith("other:"):
                other[entry] += 1
            else:
                add(entry, proj, name, 1, is_held)
        if not INHERIT.search(text):
            defs = set(DEF.findall(text))
            calls = [c for c in SELF_CALL.findall(text) if c not in defs]
            if calls:
                add(REGEX[0][0], proj, name, len(calls), is_held)
        n = sum(1 for b in mod.bodies(text) if "|" in b and "n" in [s.strip() for s in b.rsplit("|", 1)[1].split(",")])
        if n:
            add(REGEX[1][0], proj, name, n, is_held)

    print("| checklist entry | source | occurrences | files | projects | held-out files |")
    print("|---|---|---|---|---|---|")
    sources = {e: s for e, s, _ in COMPILE} | dict(REGEX)
    for entry in list(sources):
        occ, fs, ps, h = hits[entry] if entry in hits else (0, set(), set(), 0)
        print(f"| {entry} | {sources[entry]} | {occ} | {len(fs)} | {len(ps)} | {h} |")
    if other:
        print("\nother compiler errors (not an entry):")
        for k, v in sorted(other.items(), key=lambda kv: -kv[1]):
            print(f"  {v}: {k}")


if __name__ == "__main__":
    main()
