# Design: Mako evidence

Measurements before any Mako product code (method: [../development/no-reference-tool.md](../development/no-reference-tool.md), [../architecture/adding-a-language.md](../architecture/adding-a-language.md)). Scope: `*.mako` templates (Mako 1.4.3, Python). `*.mak` is not included (see Corpus). Scripts: `scripts/evidence_mako*.py|sh`, `tests/conformance/fetch-mako-corpus.sh`. Looked up 2026-11-26. Machine: AWS Neoverse-V2 aarch64, 8 cores. Tools in the slot: Python 3.12, mako 1.4.3, djlint 1.54.0 (`/scratch/slots/sudsy/6/mako/venv`).

## Corpus
1,036 `.mako` files in 830 projects (2.7 MB), pinned in `tests/conformance/fetch-mako-corpus.sh`. Files read: 1,035 (one, `sqlalchemy/mako` `test/templates/gettext_cp1251.mako`, is not UTF-8 and is left out of every count).
- Batch 1, 5 named projects pinned by commit (default-branch heads of 2026-11-26), 36 files: sqlalchemy/alembic 12, Pylons/pyramid 8, Pylons/pyramid_debugtoolbar 6, sqlalchemy/mako (Mako's own tests and examples) 6, galaxyproject/galaxy 4. The list and the threshold (a project joins with at least 3 `.mako` files in its pinned tree) were written before the counts. Counts from `scripts/evidence_mako_pins.sh`. Dropped by the threshold or absent: Pylons/pyramid_mako (2 files), Pylons/pylons (0), reddit/reddit (0), sqlalchemy/sqlalchemy (0; its docs use Sphinx), kallithea-scm/kallithea (no repository under that name).
- Batch 2, 1,000 files, one blob per item of the first 1,000 items (ten pages) of the GitHub code search `extension:mako` (124,416 hits), pinned by blob sha in `tests/conformance/mako-search.tsv` (825 repositories, written before any file was read). No batch 2 item is in a batch 1 project.
- Batch 2 was not capped by a threshold; the largest project has 22 files (cap 60 per project, `scripts/evidence_mako.py`, not reached).
- `.mak` was not searched: 276,992 hits, most of them Makefile fragments (not sampled). Decided before the count.
- Split by project, seed 20261204: deciding 415 projects, 513 files; held out 415 projects, 523 files (`python3 -I scripts/evidence.py mako list`).

## Tools
Usage evidence (`scripts/evidence_mako_usage.sh`, lookups 2026-11-26):

| Tool | Evidence | Verdict |
|---|---|---|
| djLint, `--profile mako` (djlint/djLint, PyPI 1.54.0) | 965 stars, pushed 2026-11-26; code search "djlint mako" 556 hits; `djlint` in `.pre-commit-config.yaml` 1,418 hits (not Mako-specific); a `mako` profile in its source (`djlint/formatter/mako.py`: `%` lines and `${}` masked, HTML around them formatted) | partial reference for the HTML layout only; it changes Python-text templates (see below). Not the primary tool of a Mako leg. Lead decides |
| Mako's own compiler (`mako` 1.4.3, PyPI) | the library that defines the syntax | validity reference: no formatter or linter |
| tree-sitter-mako (sbaldasty/tree-sitter-mako) | 1 star, MIT, created 2025-06-11, pushed 2025-08-13 | parser candidate (Parser candidates) |
| vsc-mako (tommorris/vsc-mako) | 5 stars, pushed 2021-01-16; not read | editor plugin, not a formatter or linter |
| djlint-vscode (djlint/djlint-vscode) | 76 stars, pushed 2026-11-25; not read | editor front end of djLint |
| mako-lint, mako-format, mako-lsp, mako-language-server (PyPI); prettier-plugin-mako, eslint-plugin-mako, mako-language-server, prettier-plugin-djlint (npm) | registry lookup: not found | none |
| pymako (PyPI) | summary "A Consul Client for Python" | not Mako: excluded |
| repository search "mako formatter", "mako linter", "mako language server", "mako syntax", "tree-sitter mako" | djLint, djlint-vscode, tree-sitter-mako, vsc-mako and unrelated repositories | no Mako formatter or linter besides djLint |
| Open VSX search | the query failed (not run) | not verified |

djLint on the corpus (`scripts/evidence_mako_tools.sh`, `--check` and `--lint` with `--profile mako`, a copy of the corpus):
- Checked 907 of 1,035 files: 128 are under paths of its default exclude list (`venv`, `node_modules`, and similar).
- Format: 864 of 907 files would change. The changes are not Mako layout: in `alembic/script.py.mako` (Python code) it removes blank lines between statements.
- Lint: 907 files, 2,415 findings. Codes: H014 (extra blank lines) 1,081, H021 334, H024 275, H025 134, H036 125, H008 77, H023 52, H043 43, H013 43. All are HTML rules.

## Format
No Mako formatter exists. Decision table (`python3 -I scripts/evidence.py mako table`): one vote per project on the deciding half (415 projects); the rule of no-reference-tool.md; agreement is the share of the held-out occurrences with the chosen form.

| Row | Votes (deciding projects) | Rule | Chosen | Held-out agreement |
|---|---|---|---|---|
| indent of a `%` control line | 101 projects: 54 indented, 47 none | as written (53% indented) | as written | n/a |
| indent of a `%` line nested in a block | 91: 90 same, 1 deeper | default | same | 76.8% (832 of 1,084) |
| indent of a text line in a `%` block | 91: 62 deeper, 23 same, 6 shallower | default + option | deeper | 77.4% (1,819 of 2,350) |
| space after `%` of a control line | 101: 63 one, 38 none | default + option | one | 58.5% (898 of 1,534) |
| space after `##` of a comment | 47: 46 one, 1 several | default | one | 65.6% (394 of 601) |
| space after `${` | 379: 375 none, 4 one | default | none | 96.8% (4,840 of 5,002) |
| space before `}` of `${` | 379: 375 none, 4 one | default | none | 96.7% (4,836 of 5,002) |
| space after `<%` of a Python block | 66: 54 newline, 12 one | default | newline | 74.4% (116 of 156) |
| space before `%>` of a Python block | 66: 54 newline, 12 one | default | newline | 74.4% (116 of 156) |
| tab in the indent of a line | 362: 356 space, 6 tab | default | space | 98.4% (18,645 of 18,948) |
| blank lines in a row | 396: 391 one, 5 two | default | one | 87.0% (4,333 of 4,979) |
| blank lines at the start of a file | 415: 412 none, 3 present | default | none | 99.4% (520 of 523) |
| blank lines at the end of a file | 415: 394 none, 21 present | default | none | 94.8% (496 of 523) |
| trailing blanks of a line | 415: 415 none | default | none | 99.8% (35,778 of 35,865) |
| final newline | 415: 390 yes, 25 no | default | yes | 91.6% (479 of 523) |
| line endings | 415: 410 LF, 5 CRLF | default | LF | 99.2% (519 of 523) |

Pooled held-out agreement over the decided rows: 74,621 of 77,769 = 96.0% (mean of the rows' project means 87.5%). Two rows are under 80% and sit at 60-80% of the vote: `%` spacing and `##` spacing, which need the option. Not done: stability (no formatter), harm sample (no formatter), meaning oracle (see Not verified).

## Lint
Checklist (`python3 -I scripts/evidence_mako_checks.py`, with a Python that has `mako` installed). The validity reference is Mako's own lexer and code generator (the first step of `Template`); the generated module is compiled by Python, not run. Mako accepts 1,030 of 1,035 files. Each entry: source, occurrences, files, projects, held-out files. The compiler stops at the first error of a file, so the compile entries count one per file.

| Checklist entry | Source | Occurrences | Files | Projects | Held-out files |
|---|---|---|---|---|---|
| control block not closed at the end of the file | lexer.py:278; syntax.rst "closed by % end" | 0 | 0 | 0 | 0 |
| end keyword with no open block | lexer.py:471 | 0 | 0 | 0 | 0 |
| end keyword does not match its block | lexer.py:476 | 0 | 0 | 0 | 0 |
| control line with no keyword | lexer.py:462 | 0 | 0 | 0 | 0 |
| tag not closed (`<%def>`, `<%block>`, `<%text>`) | lexer.py:273, 330; defs.rst | 0 | 0 | 0 | 0 |
| closing tag with no open tag, or of another tag | lexer.py:342, 348 | 0 | 0 | 0 | 0 |
| expression or `<% %>` block not terminated | lexer.py:126 | 2 | 2 | 2 | 0 |
| unknown tag name | parsetree.py:241 | 0 | 0 | 0 | 0 |
| Python syntax error in an expression or a block | pyparser.py:43 `(SyntaxError)`; Python compile | 3 | 3 | 3 | 1 |
| two defs or blocks with one name | codegen.py:1165 | 0 | 0 | 0 | 0 |
| `import *` in a Python block | pyparser.py:197 | 0 | 0 | 0 | 0 |
| namespace tag with neither name nor import | parsetree.py:415 | 0 | 0 | 0 | 0 |
| `self.NAME()` call with no def `NAME` in the file (no inherit) | defs.rst: `self` is the template's namespace | 14 | 12 | 12 | 5 |
| expression with the `n` filter (default escaping off) | filtering.rst: `n` disables the default filters | 595 | 517 | 490 | 263 |

Yield: 4 of 14 entries have findings. The 10 with none keep their source (the runtime's own messages, `lexer.py`), as no-reference-tool.md section 4.4 allows. Deprecated template syntax: none found (the four Mako docs pages have no "deprecat" text). The entries that need the project (a `<%inherit>` or `<%namespace>` target that must exist) are left out. The two regex entries are not judged: the `self` call count includes calls that Mako resolves some other way, and `n` is a review flag, not an error.

## Parser candidates
| Candidate | Evidence | Verdict |
|---|---|---|
| tree-sitter-mako (sbaldasty/tree-sitter-mako, default branch `main`, commit cff49671) | MIT; 1 star; `parser.c` (12,390 lines) and `scanner.c` (377 lines) committed; `LANGUAGE_VERSION 15`; `queries/injections.scm`: `injected_html` (combined), `injected_python_expression`, `injected_python_line`, `injected_python_block`; 15 corpus test files | candidate. Error-free share and parse speed not measured: running the compiled grammar was denied (see Not verified) |
| tree-sitter-python 0.25.0 and tree-sitter-html 0.23.2 (PyPI wheels) | the injection queries name Python and HTML as the embedded languages | the embedded regions: Python in `${ }`, `<% %>` and control lines, HTML in text (Jupyter's pattern). Not measured on Mako |
| hand-written | the lexer is small: a control line is one line (`lexer.py` 449-450), a `${` ends at the first `}` outside a string (`bodies()` in `scripts/evidence_mako.py` follows it), a `<%tag>` pair is matched by name | no build. The line-level tokenizer in `scripts/evidence_mako.py` reproduces the decision-table rows only, not a tree |

Cost of the parser in `xtask/full/languages.yaml`: ~1.2 (tree-sitter), replacing ~1.5 (hand-written).

## Not verified
- Parser: error-free share on the corpus and parse speed (tree-sitter-mako). The run of `scripts/evidence_mako_parser` (script removed from this change, not run) was denied by the auto-mode classifier; the compiled grammar was built with `cc -O2 -fPIC -shared` from the repository's `parser.c` and `scanner.c` only.
- Lint sources: the Mako docs (`syntax.rst`, `defs.rst`, `filtering.rst`, `usage.rst`) were fetched from the `main` branch on 2026-11-26; the corpus pin of sqlalchemy/mako is 14889bc0. The compile entries use the installed mako 1.4.3.
- Lint precision (50 findings judged by a reader that did not write the rule), checklist review by the lead, the bad and good examples per entry: not run.
- Format: stability, harm sample and meaning oracle: no formatter. Possible oracle for a later step: Mako's generated Python for a template must be the same before and after a formatter that only changes blanks in text.
- djLint: 128 files skipped by its default excludes were not checked; its lint findings were not judged; the Open VSX lookup failed.
- `.mak` (276,992 search hits) was not sampled; its share of Mako files is not known.
- The 1 non-UTF-8 file is left out of the corpus counts and of djLint.
