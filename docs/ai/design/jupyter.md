# Design: Jupyter Notebook evidence

Measurements before any Jupyter product code (method: [../development/no-reference-tool.md](../development/no-reference-tool.md), [../architecture/adding-a-language.md](../architecture/adding-a-language.md)). Scripts: `scripts/evidence_jupyter*.py` (facts for `scripts/evidence.py`, cell statistics, format and lint baselines, a JSON byte-splice model). Ruff 0.17.4 in a venv, Sudsy debug build with optimizations. Sudsy ignores `.ipynb` today and the LSP has no notebook support.

## Corpus
1,006 files (316 MB) of 36 projects, pinned by commit in `tests/conformance/fetch-jupyter-corpus.sh` (50 well known projects chosen before any result; 14 held no `.ipynb`: their notebooks moved to `.py` or MyST). At most 40 files per project (first by path). Split by project with seed 20261202: 18 deciding, 18 held out. 4 files are not JSON.

- nbformat: 4.4 x355, 4.5 x298, 4.2 x168, 4.0 x154, 4.1 x19, 3.x x4, other x4.
- Kernel language (files): python 827, none 55, python3 53, C# 26, R 12, PowerShell 7, Scala 6, polyglot 5, Go 4, TypeScript 2, Rust, C++ and others 4.
- Cells of Python kernels: 17,201 code, 16,453 markdown, 306 raw.
- Magics: 844 code cells (4.9%) in 444 files (line `%` 425, shell `!` 286, cell `%%` 159, help `?` 8).
- Cells that do not parse as Python: 703 (4.1%); 54 (0.31%, 43 files) once magic lines are set aside.
- File size: median 17 KB, p90 534 KB, p99 6.4 MB, max 20.9 MB; 61 files over 1 MB. Outputs are 1% of the bytes at the median, 27% on average; source is 5.4% of all bytes (at most 0.5% in the 11 files over 5 MB).
- JSON layout (deciding half): final newline 16 of 18, indent one space 17 of 18, keys sorted 17 of 18, `source` as list of lines 18 of 18, non-ASCII mixed (11 ASCII only, 6 raw, 1 escaped).

## Tools
| Tool | Evidence | Verdict |
|---|---|---|
| ruff 0.17.6 (MIT) | 442M PyPI downloads a month, 51.5k stars, pushed 2026-11-19; notebooks on by default since 0.6.0; `ruff-pre-commit` in 100k `.pre-commit-config.yaml` files; VS Code extension 5.1M installs | primary for both legs |
| black[jupyter] 26.10.1 (MIT) | 114M downloads, 42.2k stars; `black-jupyter` hook 2,136 files; VS Code 7.8M installs | second reference for format; agrees with ruff on the code cells of 635 of 820 files (77%) |
| nbQA 1.10.0 (MIT) | 904k downloads, 1.2k stars; hooks 2,280 (black 1,468, isort 1,404, ruff 581) | runs other tools on cells through a `.py` conversion; not a reference of its own |
| nbstripout 0.9.2 (MIT) | 2.4M downloads, 5,608 hook files | strips outputs, adds cell ids; language independent; no code formatting |
| nbformat 5.11.1 (BSD-3), jupytext 1.19.7 (MIT) | 96M and 3.9M downloads | JSON writers and a `.py` conversion; no style opinion on code |
| nbdime, flake8-nb, nbclean, pylint, isort, autopep8 | nbdime 1 hook file, flake8-nb 29 stars (last push 2024), nbclean last release 2018 | not references |

Behaviour (tested): ruff `format` and `check` skip a notebook whose kernel is not Python without a message; one syntax error in any cell aborts the whole format (nothing written); `%%bash` cells are left alone, `%%time` cells are formatted; black rewrites `y?` to `?y`; nbQA black ignores non-Python notebooks; ruff, black and nbstripout write indent 1, sorted keys, raw non-ASCII, a trailing newline.

Ruff only handles nbformat 4. Valid cells: code cells of a Python notebook (`language_info.name`, else `kernelspec.language`, else Python) that are not magic cells (`%%` other than capture, debug, ipytest, prun, pypy, python, python3, time, timeit). Format works per cell; lint concatenates the valid cells (each followed by a newline) and numbers cells by their raw index. Ruff's file writer is lossy: indent 1, keys sorted, `\uXXXX` escapes written raw, cell ids added for nbformat 4.5, null fields dropped (`codemirror_mode`, 3 files), float digits changed (one plotly output, 279 values).

## Target
Format: a file is equal when the `source` of every code cell equals ruff's. Every byte outside the changed `source` values stays as written (Sudsy never re-lays the JSON out). Ruff's re-layout is recorded as `choice` (layout kept) and `tool-wrong` (lost fields, float digits). Byte equality with ruff's file is not a target: it needs ruff's lossy writer. A splice model (original bytes, only changed `source` values replaced as a list of lines at the file's indent) is byte-identical to ruff's file in 545 of 661 changed notebooks (82%); the other 116 differ in layout only (indent, key order, escapes, cell ids, string to list, compact arrays).

Notebook against `.py` (what the Python rules must know in cell mode):
- A `;` after the last statement of a cell is kept and counts in the width (removed in `.py`).
- The first string of a cell is not a module docstring.
- IPython syntax (`%`, `!`, `?`, cell magics) parses; magic cells are left alone.
- Lint: E402 import boundary and I001 import blocks reset per cell; E703 allows a final `;`; B018 and B015 exempt the last expression of a cell; F704 and PLE1142 allow top-level `await`; `display` and `get_ipython` are builtins; D100 skipped; E30x and W391 per cell; F811 reported across cells.
- Non-Python notebooks (13% of files) are skipped silently, as ruff does.

Lint: the ruff default set (`--isolated`) on the cells with the `python/ruff` preset, coverage by the repo's definition.

## Baseline (estimated by scripts, not by product code)
Cells are extracted to `.py` (IPython escape lines replaced by placeholders), formatted and linted by the Sudsy Python leg, compared with ruff's notebook output. Measured: the numbers below. Estimated: the cover steps (each cause assumed fixable to equal) and the 85% joint cover.

Format: 820 Python notebooks, 15,529 cells, Sudsy formats them in about 7 s. 14,658 cells equal; 726 of 737 magic cells equal. Files with every cell equal: 569 of 820 (69.4%; deciding 64.9%, held out 73.8%).

| Cause | Cells | Files | Cumulative file cover |
|---|---|---|---|
| Notebook: final `;` kept | 647 | 161 | 84.6% |
| Python: trailing comments around brackets | 70 | 45 | 89.1% |
| Python: f-strings | 52 | 33 | 92.8% |
| Python: line breaks and parentheses | 40 | 28 | 96.2% |
| Python: multi-line string argument | 39 | 13 | 97.8% |
| Python: blank lines | 15 | 11 | 99.1% |
| Other (placeholder artifacts 2, first-string docstring 2, other 3) | 7 | 6 | 100% |

The notebook-only part is one rule (about 15 points) plus the first-string case; the rest is the Python formatter's own gaps, which any `.py` parity work closes. Format reaches 85% with the semicolon rule alone (deciding 82.8%, held out 86.5%), 90% with trailing comments.

Lint: 877 Python nbformat-4 notebooks, ruff reports 3,321 findings in 691 files. Coverage 87.9% (deciding 86.8%, held out 89.2%) with the cells linted as one module: the bar is already met. Missing findings: 369, mostly planned rules (PLR0402, RUF100, UP039, ...); 33 notebook-only (I001). Extra Sudsy findings:

| Cause | Findings | Files |
|---|---|---|
| B018/B015 on the last expression of a cell | 1,418 | 267 |
| Planned or deferred rules | 344 | 179 |
| I001 blocks not ended at a cell | 244 | 165 |
| F704 top-level `await` | 189 | 47 |
| Python: extra finding | 105 | 37 |
| Python: mapped rule misses | 25 | 19 |
| F821 `display`, `get_ipython` | 22 | 13 |

Files with no missing and no extra finding: 359 of 877 (40.9%); with the notebook causes fixed 75.1%; with the Python causes fixed 55.5%; with both 100%. Joint cover to 85% per leg: format needs the semicolon rule; lint needs last-expression, import-block, `await` and builtins rules plus the planned Python rules behind the extras.

Not verified: a Rust splice speed, the integrated pipeline, LSP behaviour, nbQA on the corpus, hand reading beyond about 30 cells, causes of the black and ruff disagreements.

## Performance
A 20 MB notebook through the existing JSON grammar: lint 0.24 s (138 MB), format 0.29 to 0.32 s (180 MB), about 70 MB/s and a tree about 7 times the file. Ruff on the same file: format 0.24 s (78 MB), check 0.18 s (40 MB). A median file costs under 0.06 s. A scanner that skips outputs would touch at most 1% of the bytes of the large files. Not measured for the Rust pipeline.

## Parts
Exist: `lang-json` parser and formatter; embedded regions (`embed` grammar rows, `KeyEmbeds` with one user, `Embedded` with carved parts, `strip`, `to_host`, region format write-back, host suppression); the Python provider on `ruff_python_parser` (it has `Mode::Ipython`, unused); the core-format safety check; the parity framework with `tool-wrong` and `choice`.

Missing:
- M1: an escape-decoded region (a JSON string array as virtual text with a piecewise offset map; write-back in the file's indent, escape style and list form).
- M2: a region group (all cells as one Python module with cell boundaries; findings mapped to cells; a fix across cells dropped).
- M3: cell-aware rules (boundaries visible to rules; Python data: builtins, last expression, final `;`, no docstring, `await` allowed).
- M4: IPython syntax in the Python parse (`Mode::Ipython`, an escape-command node kind, magic cells as non-Python regions).
- M5: a regions-only host with a safety check (bytes outside `source` identical, per-cell Python check, cell count and types unchanged).
- M6: LSP `notebookDocument` sync (cells arrive as text documents; no JSON mapping there).
- M7 (optional): a skip-subtree scan for outputs.

## Options
- A (recommended): the JSON grammar as host, Python cells as embedded regions. `KeyEmbeds` gets its second user and becomes grammar rows (two-language rule). Builds M1 to M5 in shared crates.
- B: an own adapter or scanner. Faster, but hand-written mechanism in a language crate against the HARD RULE; at most a later fast path (M7).
- C: `serde_json`. No spans, so the lossy rewrite of ruff follows. Rejected.
- D: tree-sitter-json. Adds nothing over `lang-json`. Rejected.

A syntax error in a cell: ruff aborts the format and lints the other cells. Embedding leaves such a region as written; for lint, blanking the broken cell in the concatenation is an option (not measured).

The parser choice is the user's decision: this document stops at the recommendation.

## Cost
`languages.yaml`: parser 0.2 (JSON and Python exist, only a semantic layer is new), format 3 missing parts (M1, M4, M5 plus the semicolon rule), lint 4 (M2, M3, M4, M6); estimates kept.
