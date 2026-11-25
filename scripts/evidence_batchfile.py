"""Facts of the Batchfile evidence (docs/ai/design/batchfile.md) for scripts/evidence.py and scripts/evidence_lint.py: corpus, split, the rows of the decision table, the primary linter."""
import json
import os
import re
from collections import Counter

SEED = 20261201
CAP = 40
# debugger command files of LLVM OpenMP tests: `.cmd` by name, not Batch
SKIP = re.compile(r"/openmp/libompd/test/")
DIRS = [("batchfile", re.compile(r"^.*\.(bat|cmd)$", re.I))]
PROJECT = {"runtime": "dotnet-runtime", "aspnetcore": "dotnet-aspnetcore", "git": "git-for-windows", "terminal": "windows-terminal"}
KLASSES = {}
KEEP_EOL = True  # the line endings are a row
from evidence_batchfile_rules import FAMILY, RULE  # noqa: E402,F401  (the lint verdicts)

# Blinter 1.1.74 (PyPI), no configuration; JSON on stdout
LINT = [os.environ.get("BLINTER", "blinter"), "--no-config", "--format", "json"]


def findings(out, path):
    return [{"line": i["line"], "code": i["code"], "name": i["name"], "severity": i["severity"], "message": i.get("context") or i["explanation"], "context": i.get("context", "")}
            for i in json.loads(out)["issues"]]


# row -> the variant (or variants) a formatter would write: the rule of no-reference-tool.md section 3 on the deciding half
# (filled in from the votes, see docs/ai/design/batchfile.md); None: kept as written
SUDSY = {
    "line endings (per file)": None,
    "final newline (per file)": "yes",
    "trailing blanks (per line)": "none",
    "blank lines between statements": "1",
    "indent characters (per indented line)": None,
    "block body indent (per level)": None,
    "closing parenthesis indent": "same as opener",
    "label column": "column 0",
    "command word case (per word)": "lower",
    "command word case (per file)": None,
    "blanks around == of IF": None,
    "blank before the opening parenthesis": "one blank",
    "else line": "`) else (` or `) else if`",
    "blank after REM": "blank",
    "blank after ::": "blank",
    "blanks around a redirection": None,
    "continued line indent": None,
}

WORDS = {"set", "if", "else", "for", "goto", "call", "echo", "exit", "setlocal", "endlocal", "rem", "shift", "pushd", "popd", "cd", "del", "copy", "move",
         "mkdir", "md", "rd", "rmdir", "type", "pause", "cls", "title", "color", "start", "xcopy", "robocopy", "findstr", "find", "ver", "path", "dir", "ren"}
FIRST = re.compile(r"^[ \t]*[@]?([A-Za-z]+)\b")
MASK = re.compile(r'"[^"\r\n]*"|%[^%\s]*%|![^!\s]*!|\^.')


def mask(s):
    """The line with strings, expansions and carets replaced by `Q` of the same length."""
    return MASK.sub(lambda m: "Q" * len(m.group(0)), s)


def indent_of(s):
    return s[: len(s) - len(s.lstrip(" \t"))]


def case_of(w):
    return "lower" if w.islower() else "UPPER" if w.isupper() else "Title" if w[:1].isupper() and w[1:].islower() else "mixed"


def step(opener, body):
    if not body.startswith(opener):
        return "other"
    extra = body[len(opener):]
    return "none" if not extra else "1 tab" if extra == "\t" else f"{len(extra)} spaces" if set(extra) == {" "} else "other"


def variants(text):
    rows = {r: [] for r in SUDSY}
    rows["final newline (per file)"].append("yes" if text.endswith("\n") else "no")
    n_lf, n_crlf = text.count("\n"), text.count("\r\n")
    rows["line endings (per file)"].append("none" if n_lf == 0 else "crlf" if n_crlf == n_lf else "lf" if n_crlf == 0 else "mixed")
    lines = [l.rstrip("\r") for l in text.split("\n")]
    if lines and lines[-1] == "":
        lines.pop()
    stack, run, cases, prev_cont = [], 0, Counter(), None
    for i, l in enumerate(lines):
        if not l.strip():
            run += 1
            continue
        if run and i - run > 0:
            rows["blank lines between statements"].append("1" if run == 1 else "2" if run == 2 else "3 or more")
        run = 0
        ind = indent_of(l)
        s = l.strip()
        rows["trailing blanks (per line)"].append("trailing" if l != l.rstrip(" \t") else "none")
        if ind:
            rows["indent characters (per indented line)"].append("tab" if set(ind) == {"\t"} else "spaces" if set(ind) == {" "} else "mixed")
        if prev_cont is not None:
            rows["continued line indent"].append(step(prev_cont, ind) if ind.startswith(prev_cont) else "less")
            prev_cont = None
        low = s.lstrip("@").lower()
        m = FIRST.match(l)
        w = m.group(1) if m else ""
        if low.startswith("::"):
            rows["blank after ::"].append("blank" if len(s) > 2 and s[2] in " \t" else "none")
            continue
        if low.startswith("rem") and (len(low) == 3 or not low[3].isalnum()):
            if len(low) > 3:
                rows["blank after REM"].append("blank" if low[3] in " \t" else "none")
            cases[case_of(s.lstrip("@")[:3])] += 1
            rows["command word case (per word)"].append(case_of(s.lstrip("@")[:3]))
            continue
        if s.startswith(":") and not s.startswith("::"):
            rows["label column"].append("column 0" if not ind else "indented")
            continue
        ms = mask(s)
        closes = ms.startswith(")")
        opens = ms.rstrip().endswith("(") and len(ms.rstrip()) > 1 or ms.rstrip() == "("
        if closes and stack:
            op = stack.pop()
            rows["closing parenthesis indent"].append("same as opener" if ind == op else "indented" if ind.startswith(op) and len(ind) > len(op) else "other")
        if closes and re.match(r"\)\s*else\b", s, re.I):
            rows["else line"].append("`) else (` or `) else if`" if re.match(r"\) else( \(|\s+if\b)", s, re.I) else "other spelling")
        elif not closes and stack and not s.startswith(")"):
            rows["block body indent (per level)"].append(step(stack[-1], ind))
        if w.lower() in WORDS:
            cases[case_of(w)] += 1
            rows["command word case (per word)"].append(case_of(w))
        if low.startswith("if ") or low.startswith("if/"):
            j = ms.find("==")
            if j > 0:
                b, a = ms[j - 1].isspace(), ms[j + 2 : j + 3].isspace()
                rows["blanks around == of IF"].append("blank each side" if a and b else "none" if not a and not b else "one side")
        if opens and len(ms.rstrip()) > 1:
            rows["blank before the opening parenthesis"].append("one blank" if ms.rstrip()[-2:-1] == " " and ms.rstrip()[-3:-2] != " " else "several" if ms.rstrip()[-2:-1] == " " else "none")
        if opens:
            stack.append(ind)
        if not low.startswith("echo") or ">" in ms:
            for mm in re.finditer(r"(?<![0-9&>])(\s?)(\d?)(>>?)(?![&>])(\s?)", ms):
                rows["blanks around a redirection"].append(("blank before" if mm.group(1) else "no blank before") + (", blank after" if mm.group(4) else ", none after"))
        if ms.rstrip().endswith("^") and not low.startswith(("rem", "::")):
            prev_cont = ind
    if cases:
        rows["command word case (per file)"].append("one case" if len(cases) == 1 else "mixed")
    return rows
