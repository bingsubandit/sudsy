# Mako (`lang-mako`)

## Parser
Hand-written lexer and parser on `core_syntax::parse` (`crates/lang-mako/src/lexer.rs`, `parser.rs`; evidence and decisions in [../design/mako.md](../design/mako.md)). Files: `*.mako` (`*.mak` is not claimed).

Tree: control lines (`% if`), `##` comments and `${}` and `<% %>` are flat items; control blocks are paired by `core_syntax::block_pair` (`blocks.rs`), tag pairs are nested `Element` nodes (`Tag`, `Body`, `CloseTag`; `Body` is a scope, `<%def>` and `<%block>` names are `DefName` nodes). Embedded: Python of `${}`, `<% %>` and control lines (rows with `no-lint`: Mako context names make Python's name rules noise; a block's line margin is stripped) and the HTML around the Mako parts (HTML rules run; a `${}` in an attribute value reads as blanks there, see `tests/parity/mako/differences.yaml`).

## Formatter
A template of a text host other than HTML or XML (`script.py.mako`, the `format-hosts` list of lang.yaml) is left as written: its blanks may carry meaning.
Rows of `format.rules` on the line stream (`source-indent`): trailing blanks, blank-line runs, leading and trailing blank lines, tabs in the indent, `${expr | filters}` without blanks inside the braces, one blank after `%` and `##`, final newline. Indent of block lines and the blanks of `<% %>` stay as written (named gaps). No reference tool: the decisions are the table of [../design/mako.md](../design/mako.md); held-out agreement 96.0% of occurrences, `second-format` stable on every parsed file.

## Lint
Syntax errors of the parser (`unclosed-block`, `unpaired-closer`, `invalid-control-line`, `unclosed-tag`, `unpaired-close-tag`, `unterminated-expression`, `unknown-tag`), the rules `no-wildcard-imports` (the shared Python rule, on at `error` in `<% %>` blocks through `embedded-rules` in lang.yaml), `namespace-needs-name`, `no-duplicate-def` (names local to a tag body) and, off by default, `no-default-escape-bypass` (the `n` filter). Checklist 12 of 14 entries; precision sample in `tests/parity/mako/precision.md`.

## Options
<!-- generated:options -->
| Option | Values | Default | Description |
|---|---|---|---|
| `indent_size` | int 1-16 | `4` | Columns per indentation level (also the width of a tab). |
| `final_newline` | bool | `true` | End the file with a newline. |
| `end_of_line` | `auto`, `lf`, `crlf` | `auto` | Line break style of the output: `lf`, `crlf`, or `auto` to keep the file's first line break. |
| `max_blank_lines` | int 0-10 | `1` | Largest number of blank lines kept between lines. |
| `indent_conversion` | `as_written`, `spaces`, `tabs` | `spaces` | Indentation of a line: as written, or tabs turned into spaces, or spaces into tabs (a tab is `indent_size` columns). |
| `space_after_percent` | bool | `true` | At least one blank after the `%` of a control line (`% for x in y:`); more blanks stay as written. |
| `space_after_hashes` | bool | `true` | At least one blank after `##` before a comment that starts with a letter or digit (more blanks stay as written). |
| `keep_leading_blank_lines` | bool | `false` | Blank lines at the start of the file stay. |
<!-- /generated -->
