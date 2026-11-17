# PHP (`lang-php`)

## Parser
mago-syntax (PHP 8.0 to 8.4 and the 8.5 syntax it knows: property hooks, asymmetric visibility, the pipe operator, `(void)`; a hand-written parser, pinned `=1.59.0` with its helper crates, bumped by hand: a release newer than 1.59.0 needs a newer rustc) behind `crates/lang-php/src/adapter.rs`. mago's lexer gives every token of the file (inline HTML, `<?php`/`<?=`/`?>`, the parts of interpolated strings and heredocs); the adapter drops whitespace and comments, which `provider_api::adapter::replay` rebuilds from the text between tokens ([../architecture/adding-a-language.md](../architecture/adding-a-language.md)). mago's CST gives the node spans in one walk: `grammars/php.grammar` has a row per struct of the CST, none for its enum wrappers (`Statement`, `Expression`, `Hint`, ...) or its one-token nodes (`Keyword`, names), which are tokens of the parent. Only `mago-syntax` and the crates it needs are dependencies (`third_party/mago-reporting` and `third_party/mago-php-version` replace two of them through `[patch.crates-io]`: the originals pull in schemars with `preserve_order`, which reorders every `serde_json` map of the build, the CLI's JSON output and the schema files among them; the first is the `Issue` builder mago-syntax uses, the second the original without the derive) (`mago-formatter`, `mago-linter` and `mago-analyzer` are not: layout and rules are ours). A parser panic is a parse error and the file stays as written. A syntax error loses the statement it is in (a class is one statement), so the error is reported but the tree is coarse.

Files: `*.php`, `*.phtml`, `*.inc`.

## Formatting
Not formatted yet: the file is kept as written.

## Conformance
`tests/conformance/fetch-php-corpus.sh` fetches Laravel, Symfony (with its test fixtures), WordPress and Composer at pinned commits (10,389 `.php` files, 100 MB): every file parses without error, prints back byte for byte and formats with the same tokens and idempotently (`cargo test --profile corpus -p lang-php --test integration -- --ignored --nocapture conformance`, 29 s). A file that ends in inline text (a template with no closing PHP block) keeps its end exactly: the formatter says so through `NodeFormatter::exact_end`, so no final newline is added.

## Options
<!-- generated:options -->
| Option | Values | Default | Description |
|---|---|---|---|
| `indent_size` | int 1-16 | `4` | Columns per indentation level (also the width of a tab). |
| `line_width` | int 20-1000 | `120` | Preferred maximum line width in columns. |
| `indent_style` | `space`, `tab` | `space` | Indent with spaces or tabs. |
| `final_newline` | bool | `true` | End the file with a newline. |
| `end_of_line` | `auto`, `lf`, `crlf` | `auto` | Line break style of the output: `lf`, `crlf`, or `auto` to keep the file's first line break. |
<!-- /generated -->

## Lint
`undefined-variable` (on; PHPStan level 0, [../parity/phpstan.yaml](../parity/phpstan.yaml)) is the shared scope rule on `src/scope.rs`, PHP's table: a variable is declared by its first write in its function (assignment, `list`, `foreach`, `catch`, `global`, `static`, a parameter, `use`; `use (&$x)` also writes the variable of the function around the closure), a function sees none of the names around it (an arrow function sees them), and a read with no write anywhere in the function is reported. Not decided, so not reported: a variable passed to a call (it may be taken by reference), named by `isset`, `empty`, `unset` or a static property, `$this` and the superglobals, anything at the top level of a file (an including file may define it), and every function that uses `extract`, `parse_str`, `include`, `require`, `eval` or `$$name`.
