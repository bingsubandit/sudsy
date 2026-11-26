"""Yield of the candidate lint checklist entries over the Procfile corpus (docs/ai/design/procfile.md): occurrences, files and
projects per entry, with the held-out occurrences. The patterns are the syntactic ones of the entries; each entry names its
source (the parser of a runner, or the Heroku Procfile article).
Usage: python3 -I scripts/evidence_procfile_checks.py"""
import re
import sys
from collections import defaultdict
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import evidence
import evidence_procfile as mod

NAME = r"[A-Za-z0-9_-]+"
# a process line as foreman and honcho read it: name, colon, at least one character of command after the optional blanks
ACCEPTED = re.compile(rf"^{NAME}:\s*(.+)$")
SRC_FOREMAN = "foreman lib/foreman/procfile.rb; honcho environ.py (same regex)"
SRC_OVERMIND = "overmind start/procfile.go (`:\\s+(.+)$`)"
SRC_HEROKU = "Heroku Procfile article: `<process type> is an alphanumeric name`"

# id: (test on one line with the CR removed, source); line-level entries: a match is one occurrence
LINES = {
    "name outside [A-Za-z0-9_-] (line ignored by foreman, honcho, overmind)": (
        lambda l: bool(re.match(r"^[^\s#:][^:]*:", l)) and not ACCEPTED.match(l) and not re.match(r"^\s", l) and not re.match(rf"^{NAME}:", l), SRC_FOREMAN + "; " + SRC_HEROKU),
    "leading blank before the name (line ignored by all three)": (
        lambda l: bool(re.match(r"^\s+\S", l)) and not l.lstrip().startswith("#"), "foreman/honcho/overmind `^` anchor"),
    "no blank after the colon (overmind ignores the line)": (
        lambda l: bool(re.match(rf"^{NAME}:\S", l)), SRC_OVERMIND),
    "empty command: name and colon only (ignored by foreman, honcho, overmind)": (
        lambda l: bool(re.match(rf"^{NAME}:\s*$", l)), SRC_FOREMAN + "; " + SRC_OVERMIND),
    "command continued with a trailing backslash (next line is ignored)": (
        lambda l: l.rstrip().endswith("\\"), "the three parsers: no continuation rule"),
    "line with no colon (ignored by all three)": (
        lambda l: bool(l.strip()) and not l.lstrip().startswith("#") and ":" not in l, "the three parsers"),
    "trailing blanks of a line": (
        lambda l: bool(l.strip()) and l != l.rstrip(), "style (no runtime effect)"),
    "comment line": (
        lambda l: l.lstrip().startswith("#"), "not an error: all three skip it"),
}
# file-level entries
FILES = {
    "duplicate process name (overmind fatal, honcho assert)": "overmind start/procfile.go `Process names must be unique`; honcho environ.py assert",
    "no process line (foreman EmptyFileError, overmind fatal)": "foreman lib/foreman/procfile.rb; overmind start/procfile.go",
    "CRLF line endings (style only: foreman and honcho accept)": "foreman gsub(\"\\r\\n\"); honcho splitlines",
    "no final newline": "style",
}


def file_hits(text):
    raw = text.split("\n")
    lines = [l.rstrip("\r") for l in raw]
    out = defaultdict(int)
    names = []
    for l in lines:
        m = ACCEPTED.match(l)
        if m:
            names.append(l.split(":", 1)[0])
    for cid, (test, _) in LINES.items():
        out[cid] = sum(1 for l in lines if l and test(l))
    for n in set(names):
        if names.count(n) > 1:
            out["duplicate process name (overmind fatal, honcho assert)"] += names.count(n) - 1
    if not names:
        out["no process line (foreman EmptyFileError, overmind fatal)"] = 1
    out["CRLF line endings (style only: foreman and honcho accept)"] = int("\r\n" in text)
    out["no final newline"] = int(bool(text) and not text.endswith("\n"))
    return out


corpus = evidence.files(mod)
deciding, held = evidence.split(corpus, mod.SEED)
ids = list(LINES) + list(FILES)
res = defaultdict(lambda: [0, set(), set(), 0])
for proj, name, f in corpus:
    text = evidence.read(f, True)
    if text is None:
        continue
    for cid, n in file_hits(text).items():
        if n:
            r = res[cid]
            r[0] += n
            r[1].add(name)
            r[2].add(proj)
            if proj in held:
                r[3] += n
print(f"{len(corpus)} files, {len({p for p, _, _ in corpus})} projects ({len(held)} held out)\n")
print("| entry | source | occurrences | files | projects | held-out occurrences |\n|---|---|---|---|---|---|")
for cid in ids:
    src = LINES[cid][1] if cid in LINES else FILES[cid]
    r = res.get(cid, [0, set(), set(), 0])
    print(f"| {cid} | {src} | {r[0]} | {len(r[1])} | {len(r[2])} | {r[3]} |")
