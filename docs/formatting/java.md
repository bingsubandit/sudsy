# Java (`lang-java`)

## Parser
rezel-lang-java (Java up to 26: records, sealed types, switch patterns and guards, unnamed variables, text blocks, `import module`, flexible constructor bodies; an LR parser from a Lezer grammar, pinned `=0.0.0`) behind `crates/lang-java/src/adapter.rs`, which walks rezel's tree once and builds ours with `provider_api::adapter::replay` ([../architecture/adding-a-language.md](../architecture/adding-a-language.md)); the tree shape is `grammars/java.grammar` (one row per rezel node name, loose bodies; leaves are tokens). Lezer leaves out the tokens its grammar never names (`<`, `>`, `|`, `&`, `@interface`): the adapter reads them from the text between leaves. A string or text block is one `String` token, a comment one `Comment` token (`//`, `/* */`, `/** */`). Rezel's recovery points (`⚠`) are syntax errors.

Files: `*.java`.

## Formatting
Not formatted yet: the file is kept as written.

## Options
<!-- generated:options -->
| Option | Values | Default | Description |
|---|---|---|---|
| `indent_size` | int 1-16 | `2` | Columns per indentation level (also the width of a tab). |
| `line_width` | int 20-1000 | `100` | Preferred maximum line width in columns. |
| `indent_style` | `space`, `tab` | `space` | Indent with spaces or tabs. |
| `final_newline` | bool | `true` | End the file with a newline. |
| `end_of_line` | `auto`, `lf`, `crlf` | `auto` | Line break style of the output: `lf`, `crlf`, or `auto` to keep the file's first line break. |
<!-- /generated -->
