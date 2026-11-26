# Procfile (`lang-procfile`)

Evidence, corpus, decision table and checklist: [../design/procfile.md](../design/procfile.md); no reference tool exists, so the leg follows [../development/no-reference-tool.md](../development/no-reference-tool.md).

## Parser
Hand-written line parser, shared: `core_syntax::key_value_lines` (concept `key-value-line-parser`) reads a file as lines of `key<sep>value`, comment lines and stray text; `crates/lang-procfile/src/parser.rs` only gives it the kinds of `grammars/procfile.grammar` and the bytes `#` and `:`. A line is blank (trivia), a comment (`#` after blanks, trivia), a `Process` (`Name`, `Colon`, optional `Command`; the name is the text before the first colon, as written; the blanks around the colon and at the end of the line are trivia) or a `Stray` (one `Text` token: a line with no name before a colon, also one that starts with a colon, or a line that starts with blanks, blanks included: the parser option `indented_text`). Nothing is a parse error: what runners skip is for the rules. A `#` inside a command is part of it.

Files: `Procfile` and `Procfile.*` (`Procfile.dev`).

## Formatting
The shared key-value shape (tag `#key-value` on `Process`) and the document layout: `name: command`, one blank after the colon (a command-less `name:` has none), none before; an indented line (which runners skip) is one stray text and stays as written, blanks included; blank lines between lines kept at most `max_blank_lines`, none at the start or end; no trailing blanks; LF. Names, commands, comment text (also the blank after `#`) and strays are as written, and so is a missing final newline (`keep_final_newline`: both are under 80% of the corpus).

## Options
<!-- generated:options -->
| Option | Values | Default | Description |
|---|---|---|---|
| `final_newline` | bool | `true` | End the file with a newline. |
| `keep_final_newline` | bool | `true` | A file that does not end with a line break stays without one (the final newline is not added). |
| `end_of_line` | `auto`, `lf`, `crlf` | `lf` | Line break style of the output: `lf`, `crlf`, or `auto` to keep the file's first line break. |
| `max_blank_lines` | int 0-10 | `1` | Largest number of blank lines kept between processes and comments. |
<!-- /generated -->

## Presets
No presets.

## Rules
Shared: suppression comments, `max-line-length`, `no-empty-comment`, `comment-space`.

Language rules (pattern rules; the runners are foreman, honcho and overmind, see the design page for each source):
- `invalid-process-name`: a name outside `[A-Za-z0-9_-]`, which runners skip.
- `no-duplicate-process-name` (family `unique-names`): a name listed twice.
- `stray-line`: a line with no `name:`, which runners skip.
- `no-process`: no process line in the file.
- `indented-process`: blanks before the name, which runners skip.
- `missing-space-after-colon`: `name:command`, which overmind skips.
- `empty-command`: `name:` with nothing after it.

Line endings, trailing blanks and the final newline are the formatter's, not rules.

## Conformance
`tests/conformance/fetch-procfile-corpus.sh` fetches the corpus; `cargo test --profile corpus -p lang-procfile --test integration -- --ignored --nocapture conformance` checks that each file round-trips and formats idempotently.
