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
The code cells are one program (`joined: true` of the `KeyEmbeds` row, `core_syntax::Embedded.joined`): they are parsed as one IPython-dialect Python module and the Python rules run on it; `to_host` maps each finding back into its cell. A cell that does not parse alone is left out of the program (`provider-api/src/embedded.rs` `join`). Rules see the cells through `FileContext.cells` (`core_rules::cell_index`):
- import blocks (`unsorted-imports`, `misplaced-import` and the blank lines after a block) end with their cell;
- `cell-output: true` (pattern key) is the last top-level statement of a cell: `no-useless-expression` leaves it (it is the cell's output);
- top-level `await` is allowed (`yield-outside-function`, `invalid-syntax`);
- the preset `python/ruff` sets `undefined-variable` `globals: [__IPYTHON__, display, get_ipython]` for `**/*.ipynb` (ruff's notebook builtins).

`scripts/parity.sh coverage ruff-notebook`: 2,920 of 3,390 ruff findings covered (the share is in the generated table of [../../languages.md](../../languages.md)). Misses (`missed:` of `tests/parity/ruff-notebook/differences.yaml`): rules without a Sudsy rule (PLR0402 76, RUF100 46, UP039 16, LOG015 14, PLR1736 12, RUF015 12), ruff's `invalid-syntax` in a broken cell (68: Sudsy leaves the cell out silently), I001 on a one-line `try: import x` (28), F811 across cells (15). Extra findings: about 105 (`unsorted-imports` 36, `syntax-error` 8, `assert-false` 8, `redeclared-variable` 7).

## Performance
`bench/inputs/ipynb` (3 notebooks, 115 KB). Numbers: [../performance/numbers.md](../performance/numbers.md).
