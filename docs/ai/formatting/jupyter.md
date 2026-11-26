# Jupyter Notebook (`lang-jupyter`, `.ipynb`)

A notebook is JSON written by Jupyter. `lang-jupyter` is a dialect of `json` (`with_dialect("jupyter", "json")`): `lang_json::parse_file` parses it, `VerbatimFormatter` keeps the JSON text as written (the writer lays it out), and the code cells are formatted in place as embedded Python by the embedded-format query (`core-incremental/src/embedded.rs`). The crate (`crates/lang-jupyter/src/lib.rs`, 72 lines) holds one `KeyEmbeds` row and the `embedded_language_formatting` option (the schema extends json's, so json's presets apply).

## Options
<!-- generated:options -->
| Option | Values | Default | Description |
|---|---|---|---|
| `line_width` | int 20-1000 | `80` | Preferred maximum line width in columns. |
| `indent_size` | int 1-16 | `2` | Columns per indentation level (also the width of a tab). |
| `indent_style` | `space`, `tab` | `space` | Indent with spaces or tabs. |
| `final_newline` | bool | `true` | End the file with a newline. |
| `end_of_line` | `auto`, `lf`, `crlf` | `auto` | Line break style of the output: `lf`, `crlf`, or `auto` to keep the file's first line break. |
| `bracket_spacing` | bool | `true` | Spaces inside the braces of a one-line mapping or object: `{ a: 1 }` or `{a: 1}`. |
| `array_bracket_spacing` | bool | `false` | Spaces inside the brackets of a one-line sequence or array: `[ 1, 2 ]` or `[1, 2]`. |
| `collapse` | `preserve`, `always`, `never`, `opening_break` | `preserve` | Objects and arrays on one line: `preserve` keeps one that has a line break between its brackets expanded and puts the others on one line if they fit; `always` puts every one that fits on one line; `never` puts every non-empty one on several lines; `opening_break` keeps an object expanded when a line break follows its opening brace and puts every array that fits on one line. |
| `trailing_commas` | `never`, `multiline` | `never` | Trailing commas after the last item of a multi-line object or array (JSONC only). |
| `dialect` | `auto`, `json`, `jsonc` | `auto` | JSON or JSONC (comments, trailing commas); `auto` decides by file name. |
| `embedded_language_formatting` | `auto`, `off` | `auto` | Format the code cells with the formatter of the notebook's kernel language (`auto`), or keep them as written (`off`). A cell that does not parse, a `%%` cell magic other than Python's and a notebook of another kernel language are kept either way. |
<!-- /generated -->

## Region
`KeyEmbed` row `cells.*.source` of a code cell (`when cells.$1.cell_type = code`), language Python. The kernel language is `metadata.language_info.name`, else `metadata.kernelspec.language`; a notebook with a `kernelspec` and no language is not Python (`unless`); with neither it is Python. `source` is one string or an array of strings: `core_syntax::escaped::Escaped` reads it as virtual text (the strings joined, escapes decoded) with a piecewise offset map; write-back keeps the file's array form, indent and escape style, and the last line without a newline. A cell with a `%%` magic other than Python's (`%%time`, `%%capture`, ...) is left as written (`foreign`), as is a cell that does not parse.

Cell settings (`KeyEmbed.settings`, Python options): `dialect: ipython` (`%magic`, `!cmd`, `?help` lines are `IpyEscapeCommand` nodes), `keep_final_semicolon`, `keep_statement_parentheses`, `module_docstring: false` (a first string is no docstring). The trailing newline of a cell is stripped (`format_code` with `exact`).

## Safety
Each region is formatted with its own language, then checked with `same_code` (the formatter's source rewrite followed on the before tree, then `safety::verify`) and `reads_back` (the file parses again, same cell count and types, bytes outside `source` identical).

## Parity
`scripts/parity.sh format ruff-notebook` against `ruff format` (`tests/parity/ruff-notebook/`, corpus of `tests/conformance/fetch-jupyter-corpus.sh`): cells are compared after normalizing ruff's JSON re-layout (`notebook-cells` in `scripts/parity_format.py`). 861 of 974 files identical (the generated table of [../../languages.md](../../languages.md) has the share); 32 more are rejected by ruff (a cell with a syntax error: ruff refuses the whole file); second-format 0. The 113 differences are Python formatter gaps, listed in `differences.yaml`: f-string replacement fields (35), trailing-comment width (33), a lone multi-line string argument (13), single layouts (26), blank lines (4), comment placement (2).

## Lint
`ruff check` (default set, `preset:python/ruff`) on the code cells: `scripts/parity.sh coverage ruff-notebook` (same corpus as the format run, split by project into a deciding half and a held-out half with seed 20261202). The share covered is in the generated table of [../../languages.md](../../languages.md); the deciding half and the held-out half are both above the 85% bar (measured 2026-11-26). The embedded lint (`core-incremental/src/embedded.rs`) parses the cells as one Python program (`provider-api/src/embedded.rs`: each cell followed by a newline, `EmbeddedParse.cells` the ranges, `Embedded.cell` set by the row's `cells: true`), so a name bound in one cell is known in the next, and maps findings back with the offset map; a fix that spans cells is dropped. Notebook facts:
- `FileContext.cells` makes import runs end at a cell boundary (`unsorted-imports`, `manual-from-import`: one block per cell, as ruff's I001) and `jump-outside` (`in-cells`) treat a cell as a module level.
- Rules with the option `except_cell_tail` (default true: `no-useless-expression`) skip the last statement of a cell, whose value the notebook shows (`core-rules/src/cells.rs`).
- Top-level `await` is allowed in a notebook, and `display` and `get_ipython` are builtins (`scope.rows`: `flavor notebook .ipynb`, `globals-if notebook ...`).
Gaps: `module-import-not-at-top-of-file` (E402) does not restart per cell; `unsorted-imports` reports 239 more than ruff (blank lines or comments before a definition); the missed rules (`invalid-syntax` of 4 files ruff accepts, RUF100, UP039, ...) are listed in `tests/parity/ruff-notebook/differences.yaml`.

## Performance
`bench/inputs/ipynb` (3 notebooks, 115 KB). Numbers: [../performance/numbers.md](../performance/numbers.md).
