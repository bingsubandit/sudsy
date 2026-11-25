# Batchfile (`lang-batch`)

## Parser
Hand-written lexer and parser on `core_syntax::parse` (`crates/lang-batch/src/lexer.rs`, `parser.rs`; design and measurements in [../design/batchfile.md](../design/batchfile.md)). The lexer reads a line by the command it starts: `rem` and `::` take the rest of the line, `echo`, `set` (plain), `title` and `prompt` take text whose blanks are content, `if`, `for` and `else` are keywords at the start of a command, `)` ends a word only inside a block or a `FOR` list, a `(` in an argument is balanced by its own `)`, `^` plus a line break continues the line. Files: `*.bat` and `*.cmd`. A missing `)` is one `unclosed-block` error and formatting refuses the file; so do the files that are not cmd (PowerShell or C# text after a `goto`).

## Formatting
Written line by line from the source (`source-indent`): every line keeps its indentation, a block body is not re-indented.
- Command words (internal commands) and the keywords `if else for in do rem` go lower case; programs, switches, `EQU` and the like, labels, variables and arguments keep their case.
- A labelled line stands at column 0 outside a block; a `)` that starts a line stands where the line of its `(` starts; one blank before the `(` of a block and around `) else (`.
- `rem` keeps at least one blank before its text; a comment `::` gets at least one blank before a text that starts with a letter or digit (`space_after_colons`).
- Trailing blanks go, except where they are part of the text: `set x=y `, `echo x `.
- Blank lines collapse to `max_blank_lines` (1); the file ends with one line break; line ends are kept per file (`end_of_line: auto`).
- `indent_conversion` turns the indentation into spaces or tabs; `continued_indent: none` removes the indentation of a line that continues one that ends in a blank and `^`.

## Options
<!-- generated:options -->
| Option | Values | Default | Description |
|---|---|---|---|
| `indent_size` | int 1-16 | `4` | Columns per indentation level (also the width of a tab). |
| `final_newline` | bool | `true` | End the file with a newline. |
| `end_of_line` | `auto`, `lf`, `crlf` | `auto` | Line break style of the output: `lf`, `crlf`, or `auto` to keep the file's first line break. |
| `max_blank_lines` | int 0-10 | `1` | Largest number of blank lines kept between lines. |
| `indent_conversion` | `as_written`, `spaces`, `tabs` | `as_written` | Indentation of a line: as written, or tabs turned into spaces, or spaces into tabs (a tab is `indent_size` columns). |
| `continued_indent` | `as_written`, `none` | `as_written` | Indentation of a line that continues the one before it with `^`: as written, or none when a blank stands before the `^` (the blanks then separate arguments and are not content). |
| `space_after_colons` | bool | `true` | At least one blank after `::` before a comment that starts with a letter or digit (more blanks stay as written). |
<!-- /generated -->
