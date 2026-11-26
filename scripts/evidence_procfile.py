"""Facts of the Procfile format evidence (docs/ai/design/procfile.md) for scripts/evidence.py: corpus, the rows of the decision table.

`python3 -I scripts/evidence.py procfile table` prints the decision table with the vote of the deciding projects and the
held-out agreement of the form each row names in SUDSY.

A process file is `name: command` per line (Heroku, foreman, honcho, overmind, hivemind, dokku, herokuish); a line that is
not one starts with `#` (comment) or is blank."""
import re

SEED = 20261204
NAMES = re.compile(r"^Procfile(\.(dev|local))?$")
DIRS = [("procfile", NAMES)]
PROJECT = {}
CAP = 60
KEEP_EOL = True
TOOL = None
KLASSES = {}

# row -> the variant (or variants) a formatter writes; None: kept as written (below 60% of the deciding projects, or not a layout
# choice: a process name is an identifier, the formatter must not change it)
SUDSY = {
    "blank after `:`": "one",
    "blank before `:`": "none",
    "blank after `#` of a comment": "one",
    "blank lines in a row": "one",
    "blank lines at the start of a file": "none",
    "blank lines at the end of a file": "none",
    "trailing blanks of a line": "none",
    "final newline": None,
    "line endings": "LF",
    "`\\` continuation of a process line": "single",
    "indent of a process line": "none",
    "process name case": None,
}

PROCESS = re.compile(r"^([ \t]*)([^\s:#]+)([ \t]*):([ \t]*)(\S.*)?$")


def gap(s):
    return {"": "none", " ": "one", "\t": "tab"}.get(s, "several")


def variants(text):
    v = {k: [] for k in SUDSY}
    raw = text.split("\n")
    v["final newline"].append("yes" if text.endswith("\n") else "no")
    v["line endings"].append("CRLF" if "\r\n" in text else "LF")
    v["blank lines at the start of a file"].append("present" if raw and not raw[0].strip() else "none")
    v["blank lines at the end of a file"].append("present" if len(raw) > 1 and not raw[-1].strip() and not raw[-2].strip() else "none")
    run = 0
    for k, l in enumerate(raw):
        l = l.rstrip("\r")
        if not l.strip():
            run += 1
            continue
        if run and k:
            v["blank lines in a row"].append({1: "one", 2: "two"}.get(run, "three or more"))
        run = 0
        if l != l.rstrip():
            v["trailing blanks of a line"].append("present")
        elif l.strip():
            v["trailing blanks of a line"].append("none")
        if l.lstrip().startswith("#"):
            body = l.lstrip()[1:]
            if body[:1] in (" ", "\t"):
                v["blank after `#` of a comment"].append("one")
            elif body.strip():
                v["blank after `#` of a comment"].append("none")
            continue
        m = PROCESS.match(l)
        if not m:
            continue
        ind, name, before, after, cmd = m.groups()
        v["indent of a process line"].append("indented" if ind else "none")
        v["process name case"].append("lower" if name == name.lower() else "not lower")
        v["blank before `:`"].append(gap(before))
        if cmd:
            v["blank after `:`"].append(gap(after))
        if l.rstrip().endswith("\\"):
            v["`\\` continuation of a process line"].append("continued")
        else:
            v["`\\` continuation of a process line"].append("single")
    return v
