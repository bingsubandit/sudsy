"""Facts of the Rust format evidence (docs/ai/design/rust.md) for scripts/evidence.py: corpus, the rows of the decision table.

`python3 -I scripts/evidence.py rust table` prints the decision table with the vote of the deciding projects and the
held-out agreement of the form each row names in SUDSY. The reference formatter is rustfmt (scripts/evidence_rust_tools.py);
the rows are the layout choices that rustfmt's defaults make, read from the corpus without running rustfmt, so that the
agreement of real code with each default is visible row by row.

Corpus: tests/conformance/corpus/rust (fetch: tests/conformance/fetch-rust-corpus.sh). Test-data folders (rustfmt-style
`tests/source` and `tests/target` pairs, `ui` and `test_data` fixtures, which are deliberately unformatted or do not compile)
are left out of every count (SKIP). A project keeps at most CAP files, the first by path.
"""
import re

SEED = 20261204
DIRS = [("rust", re.compile(r"^.*\.rs$"))]
SKIP = re.compile(r"/corpus/rust/.*/(ui|source|target|fixtures?|test_data|testdata|test-data)/")
PROJECT = {}
CAP = 30
KEEP_EOL = True
TOOL = None
KLASSES = {}

# row -> the variant rustfmt's defaults write (a string, or a set when two are equally right); None: kept as written
SUDSY = {
    "indent of a line": "spaces",
    "indent width of an indented line (spaces)": "multiple of 4",
    "trailing blanks of a line": "none",
    "final newline": "yes",
    "line endings": "LF",
    "blank lines at the start of a file": "none",
    "blank lines at the end of a file": "none",
    "blank lines in a row": "one",
    "blank line after a line ending with `{`": "none",
    "line width": "within 100",
    "brace of an item head with one line": "same line",
}

HEAD = re.compile(r"^\s*(pub(\([a-z]+\))?\s+)?(async\s+|unsafe\s+|const\s+|extern\s+(\"[^\"]*\"\s+)?)*(fn|struct|enum|trait|impl|union)\b")


def variants(text):
    v = {k: [] for k in SUDSY}
    raw = [l.rstrip("\r") for l in text.split("\n")]
    v["final newline"].append("yes" if text.endswith("\n") else "no")
    v["line endings"].append("CRLF" if "\r\n" in text else "LF")
    nonblank = [k for k, l in enumerate(raw) if l.strip()]
    v["blank lines at the start of a file"].append("present" if nonblank and nonblank[0] > 0 else "none")
    # the split leaves an empty last element after a final newline: two blank elements at the end are a blank line
    v["blank lines at the end of a file"].append("present" if len(raw) > 2 and not raw[-1].strip() and not raw[-2].strip() else "none")
    run = 0
    for k, l in enumerate(raw):
        if not l.strip():
            run += 1
            continue
        if run and k:
            v["blank lines in a row"].append({1: "one", 2: "two"}.get(run, "three or more"))
        run = 0
        ind = len(l) - len(l.lstrip(" \t"))
        if ind:
            v["indent of a line"].append("tab" if l[0] == "\t" else "spaces")
        if l[:ind].count(" ") == ind and ind:
            v["indent width of an indented line (spaces)"].append("multiple of 4" if ind % 4 == 0 else "other")
        v["trailing blanks of a line"].append("present" if l != l.rstrip() else "none")
        v["line width"].append("within 100" if len(l) <= 100 else "over 100")
        nxt = next((raw[j] for j in range(k + 1, len(raw)) if raw[j].strip()), None)
        if l.rstrip().endswith("{"):
            after = raw[k + 1] if k + 1 < len(raw) else ""
            v["blank line after a line ending with `{`"].append("present" if not after.strip() else "none")
        if HEAD.match(l) and not l.rstrip().endswith(("{", ";", ",", "(")):
            v["brace of an item head with one line"].append("own line" if nxt is not None and nxt.strip() == "{" else "other")
        elif HEAD.match(l) and l.rstrip().endswith("{"):
            v["brace of an item head with one line"].append("same line")
    return v
