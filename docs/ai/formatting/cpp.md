# C and C++

Two languages, `c` and `cpp`, one crate (`lang-cpp`) with one hand-written lexer and one structural layer. There is no parser for the full grammar: C and C++ cannot be parsed without the preprocessor and the types the headers declare, and the tree-sitter grammars and `cpp_parser` fail on 5-67% of real files (unknown macros). The tree is a token-level structure in the manner of clang-format's unwrapped lines, and it never fails.

Files: `.c` and `.h` are `c`; `.cc .cpp .cxx .hpp .hh .hxx .ipp` are `cpp`. A `.h` is C because the extension does not say, and the lexer differs only in keywords (C++ keywords are identifiers in C); C++ projects that use `.h` select `cpp` with a `files` override.

## Lexer (`lexer.rs`)
Lossless tokens: identifiers, keywords (by dialect), numbers (digit separators, hex floats, suffixes), strings and chars with encoding prefixes and raw strings, header names after `#include`/`#include_next`/`#import`, maximal-munch punctuators, and `Other` for a stray byte. Comments and backslash-newline continuations are trivia. A `#` that starts a line begins a directive; the directive's tokens carry its id. The only errors are an unterminated block comment or raw string.

## Structure (`structure.rs`)
One pass over the tokens builds nodes over them:
- `Line`: an unwrapped line (a statement, a declaration, a control-statement header, a `case`/label, a `public:`, a macro invocation alone on its line). A line that ends in a block holds the `BraceGroup`.
- `BraceGroup`, `ParenGroup`, `BracketGroup`: bracket groups, nested in the line that opens them.
- `Directive`: a preprocessor directive including its continuation lines; directives are siblings of lines, never inside one.
- `PpGroup` of `PpBranch`: an `#if`...`#endif` whose branches start and end at the same bracket depth. When a branch opens or closes a bracket the others do not, only the `Directive` nodes remain.

`#if` branches are not each parsed as clang-format does: at `#else`/`#elif` the groups opened in the branch are closed back to the depth at `#if`, and at `#endif` the state of the last branch continues (single-pass bracket reset). 0.2-6.5% of corpus files have branch-asymmetric groups.

## Formatter
Verbatim for now: the text is printed back unchanged. The layout is built on the shared shapes ([../design/cpp-formatter.md](../design/cpp-formatter.md)).

## Parity
`scripts/parity.sh format clang-format` runs clang-format 20.1.8 (`tests/parity/clang-format/`, a `.clang-format` per style: each corpus project's own for LLVM, fmt, range-v3 and Linux, LLVM for folly and SQLite, the six presets on the bench inputs; sorting of includes, namespace end comments and comment reflow off) and compares it with `sudsy format` on the corpus of `fetch-cpp-corpus.sh`. Identical files of the verbatim formatter (the ratchets of `differences.yaml`, which may only rise): LLVM 227/420, folly 367/2,299, Linux 312/2,872, range-v3 49/593, fmt 20/70, SQLite 0/325, 0/10 for each preset. Second format and refused: 0 in every project.

## Options
<!-- generated:options -->
| Option | Values | Default | Description |
|---|---|---|---|
| `line_width` | int 20-1000 | `80` | Preferred maximum line width in columns. |
| `indent_size` | int 1-16 | `2` | Columns per indentation level (also the width of a tab). |
| `indent_style` | `space`, `tab` | `space` | Indent with spaces or tabs. |
| `final_newline` | bool | `true` | End the file with a newline. |
| `end_of_line` | `auto`, `lf`, `crlf` | `auto` | Line break style of the output: `lf`, `crlf`, or `auto` to keep the file's first line break. |
| `max_blank_lines` | int 0-10 | `1` | Largest number of blank lines kept between statements and comments. |
| `wrap_lines` | bool | `true` | Break lines that are longer than `line_width`. When false, every line keeps the line breaks of the source (only indentation and spacing change). |
| `continuation_indent` | int 0-16 | `4` | Columns a wrapped line is indented more than the line it continues, where it is not aligned to a bracket. |
| `access_modifier_outdent` | int 0-16 | `2` | Columns `public:`, `private:` and `protected:` are outdented from the members of their class. |
| `indent_case_labels` | bool | `false` | Indent `case` labels one level inside their `switch`. |
| `namespace_indentation` | `none`, `inner`, `all` | `none` | Indent the contents of namespaces: `none`, only namespaces nested in another (`inner`), or `all`. |
| `pointer_alignment` | `right`, `left`, `middle` | `right` | Where the `*` and `&` of a pointer or reference declarator go: next to the name (`right`), next to the type (`left`) or between them (`middle`). |
| `brace_wrapping` | list of words | `[]` | Constructs whose opening brace goes on its own line: `class`, `struct`, `union`, `enum`, `function`, `namespace`, `extern`, `control` (if, for, while, switch, do, try), `case`, `else` (the `else` after a block starts a line), `catch` (the `catch` after a block starts a line), `while` (the `while` of a do loop starts a line). Empty: every brace stays on the line of its construct. |
| `indent_braces` | bool | `false` | Indent a brace that is on its own line together with the block it opens. |
| `short_functions` | `none`, `empty`, `inline`, `all` | `all` | Function bodies kept on the line of the signature when they fit: `none`, `empty` (`{}` only), `inline` (functions defined in a class, and empty ones) or `all`. |
| `short_ifs` | `never`, `without_else`, `all` | `never` | An `if` with a simple statement is kept on one line when it fits: `never`, without an `else`, or with or without. |
| `short_loops` | bool | `false` | A `for`, `while` or range-for with a simple statement is kept on one line when it fits. |
| `short_lambdas` | `none`, `empty`, `inline`, `all` | `all` | Lambda bodies kept on the line of the lambda when they fit: `none`, `empty`, `inline` (a lambda that is an argument) or `all`. |
| `short_case_labels` | bool | `false` | A `case` label and its simple statement are kept on one line when they fit. |
| `bin_pack_arguments` | bool | `true` | Call arguments that do not fit on a line are filled; when false they go one per line (or all on one line). |
| `bin_pack_parameters` | bool | `true` | Function parameters that do not fit on a line are filled; when false they go one per line (or all on one line). |
| `align_after_open_bracket` | `align`, `dont_align`, `always_break` | `align` | Wrapped lines inside brackets line up with the first token after the bracket (`align`), continue with `continuation_indent` (`dont_align`), or always start on a new line after the bracket (`always_break`). |
| `break_before_binary_operators` | `none`, `non_assignment`, `all` | `none` | A wrapped binary expression breaks after the operator (`none`), before it except for assignments (`non_assignment`) or before every operator (`all`). |
| `break_constructor_initializers` | `before_colon`, `before_comma` | `before_colon` | Constructor initializer lists that do not fit start on a new line before the colon (`before_colon`, one initializer per line when needed), or put a comma before each initializer, one per line (`before_comma`). |
| `constructor_initializer_indent` | int 0-16 | `4` | Columns a constructor initializer list is indented more than its constructor. |
| `break_after_return_type` | `none`, `top_level`, `all`, `top_level_definitions`, `all_definitions` | `none` | The function name goes on a new line after its return type: never, for declarations and definitions outside classes (`top_level`), for all, or for definitions only (`top_level_definitions`, `all_definitions`). |
| `short_enums` | bool | `true` | An enum whose body fits stays on one line. |
| `penalty_return_type` | int 0-1000 | `60` | What a return type on a line of its own costs the layout; the higher, the longer the parameter list may wrap before the return type moves. |
| `space_before_parens` | `never`, `control_statements`, `always` | `control_statements` | A blank before the parenthesis of `if`, `for`, `while`, `switch` and `catch` (`control_statements`), before every parenthesis (`always`) or never. |
| `spaces_before_trailing_comments` | int 0-16 | `1` | Blanks between the code of a line and its `//` comment. |
| `align_trailing_comments` | bool | `true` | Align the trailing comments of consecutive lines. |
| `break_template_declarations` | `yes`, `multiline`, `no` | `multiline` | `template <...>` ends its line: always (`yes`), when the declaration is not on one line (`multiline`) or never. |
| `align_escaped_newlines` | `right`, `left`, `dont_align` | `right` | The backslashes of a multi-line macro: aligned at `line_width` (`right`), one blank after the longest line (`left`) or one blank after each line. |
<!-- /generated -->

## Conformance
The pinned projects of `tests/conformance/fetch-cpp-corpus.sh` (Linux kernel, SQLite, LLVM, folly, fmt, range-v3; 6,594 files): every file parses without errors, prints back unchanged and passes the shared checks. `cargo test --profile corpus -p lang-cpp --test integration -- --ignored --nocapture conformance` after `bash tests/conformance/fetch-cpp-corpus.sh`; `structure_stats` in the same way prints speed and the shape of the structure per project.
