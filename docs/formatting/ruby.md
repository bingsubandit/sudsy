# Ruby (`lang-ruby`)

## Parser
Prism (the parser of CRuby 3.4 and later, C, vendored in `ruby-prism-sys` pinned `=2.1.0`, bumped by hand; Ruby 3.3 to 4.0 syntax) behind `crates/lang-ruby/src/adapter.rs`, used for parsing only. It was chosen over tree-sitter-ruby (5 failing real files, a failing 3.3 form, 17x slower) and lib-ruby-parser (Ruby 3.1 syntax, stalled): Prism parsed all 18,958 `.rb`/`.rake` files of the corpus and every probe of the newest syntax, at 124-178 MB/s. The Rust crate `ruby-prism` is not used: its AST wrappers give no tokens, and its visitor skips the hook for typed fields (`ProgramNode.statements`, `CallNode.arguments`), so the adapter declares three C entry points by hand next to the sys crate's bindings and gets everything from one parse:
- `pm_parser_lex_callback_set`: every token in lex order, which `provider_api::adapter::replay` takes as the parser's tokens ([../architecture/adding-a-language.md](../architecture/adding-a-language.md)); whitespace, newlines and comments (`#`, `=begin`/`=end`, a `\` that ends a line) are rebuilt from the text between them.
- `pm_visit_node`: the span of every node, one row of `grammars/ruby.grammar` per Prism node type, named as Prism names it (`CallNode`), so the table is found by name.
- `pm_parser_errors_first` and `pm_parser_data_loc`: the diagnostics and the `__END__` line.

`src/adapter/prism.rs` is the only unsafe code (the crate sets `unsafe_code = "deny"` and allows it in that module). A syntax error is reported with Prism's message and the file stays as written; a panic is a parse error too. Every engine parse goes through `provider_api::guard::parse` (bracket nesting limit, panics). Prism itself recurses up to 10,000 levels on the C stack (`!!!...x`, nested `def` or `if`, which the bracket limit does not count), which a debug build cannot hold on a 2 MB thread: the parse runs on a 32 MB stack segment (`stacker`) when less than 16 MB is left, and a file nested past Prism's limit gets its errors.

Where Prism's spans do not nest, the adapter fixes them (all of it in `adapter.rs`):
- A heredoc's node covers only its opener (`<<~EOS`); its body and closing label are lexed right after it. The body, with the interpolations inside it, is one `HeredocBody` token (`#optional`, so bodies do not fit any node body) after the line of the opener, a child of the innermost node still open there; the nodes inside the body are left out. The code in the `#{}` of a heredoc is therefore not in the tree.
- `__END__` is a `Keyword` and the text after its line one `Data` token, which a final newline never touches (`NodeFormatter::exact_end`).
- The body of a `def`, block or class that has `rescue` or `ensure` is a `BeginNode` with the span of the whole construct; it is shortened to what its children cover, so the `def` and `end` keywords stay with the `def`.
- A hash shorthand (`{ x: }`) has a value node with the span of the key: it and its parts are left out. The local variables a regexp with named captures assigns (`/(?<a>.)/ =~ s`) have the span of the regexp and are left out.

Files: `*.rb`, `*.rake`, `*.gemspec`, `*.ru`, `Gemfile`, `Rakefile`.

## Grammar
151 node kinds, 35 token kinds. Every keyword is `Keyword`, every operator without a token of its own `Operator`. The grammar opts in to `roles` ([../architecture/syntax-tree.md](../architecture/syntax-tree.md)): 85 kinds have a role with its fields, 66 are `#opaque`. Roles by Ruby's own shapes:
- Everything with a receiver or a name is a `CallNode` (`a + b`, `a[1]`, `a.b = 1`, `not a`, `-a` too), so it is `#call` and `#member-access`, not `#binary` or `#index`; `AndNode`, `OrNode`, ranges and flip-flops are `#binary`, `defined?` is `#unary`. The conditional forms (`if`, `unless`, ternary: one `IfNode`), loops, `case`, `begin`, `rescue` and the other statements are `#opaque` until the role vocabulary has them.
- Literals (numbers, strings, symbols, regexps, arrays, hashes) are `#literal`; reads of names `#reference`; the write nodes (`a = 1`, `@a ||= 1`, `A::B = 1`, `a, b = 1, 2`) `#assignment`; `def`, `->`, blocks `#function` and `#scope`; `class`, `module`, `class << self` `#type-declaration`; parameters `#declaration`. Fields of a write node are the name token or target and the value, whatever the operator.
- A body lists children in source order, so a node whose source order differs from Prism's field order (modifier `if`, `while`) is `#opaque`. `#expression` is on every node that can stand as a value, `#body` on the nodes that hold statements (`StatementsNode`, `BeginNode`).

## Formatting
Not formatted yet: the file is kept as written.

## Conformance
`tests/conformance/fetch-ruby-corpus.sh` fetches Rails (`a5203b3`), Discourse (`47f9840`), RuboCop (`dacb36c`) and Jekyll (`97c3bdc`) at pinned commits (18,989 `.rb`, `.rake`, `.gemspec` and `.ru` files): every file parses without error, prints back byte for byte, fits the bodies of the grammar (`ShapeChecker`) and formats with the same tokens and idempotently (`cargo test --profile corpus -p lang-ruby --test integration -- --ignored --nocapture conformance`).

## Reference tools for the next steps
- Format: RuboCop's Layout cops (`rubocop -a --only Layout`, the style most teams run), Standard (RuboCop with fixed config), syntax_tree (`stree`, prettier-like, the closest to a pure formatter), rufo.
- Lint: RuboCop's Lint cops, then the file-local Style and Naming cops.
- No Ruby is installed: the parity runner needs one (a prebuilt portable Ruby as a release asset with the gems installed by `gem install --install-dir`; not tried).

## Options
<!-- generated:options -->
| Option | Values | Default | Description |
|---|---|---|---|
| `indent_size` | int 1-16 | `2` | Columns per indentation level (also the width of a tab). |
| `line_width` | int 20-1000 | `120` | Preferred maximum line width in columns. |
| `indent_style` | `space`, `tab` | `space` | Indent with spaces or tabs. |
| `final_newline` | bool | `true` | End the file with a newline. |
| `end_of_line` | `auto`, `lf`, `crlf` | `auto` | Line break style of the output: `lf`, `crlf`, or `auto` to keep the file's first line break. |
<!-- /generated -->
