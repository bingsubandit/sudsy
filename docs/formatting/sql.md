# SQL (`lang-sql`)

## Parser
One hand-written recursive-descent parser (`crates/lang-sql/src/parse/`) reads `ansi` (default), `postgres`, `mysql` and `tsql` as modes of itself (`dialect.rs`; the data tables `parse/data/`: keywords per dialect, reserved words, the statement table). One language `sql` covers `*.sql`; the dialect is the option `dialect`; it travels in `SourceFile::language_version`. Other dialects are not offered.

- `lexer.rs` tiles the file with tokens (whitespace, comments, words, quoted names, strings with their dialect forms such as dollar quotes and `U&''`, numbers, variables, operators, `DELIMITER` in MySQL, `COPY .. FROM STDIN` data and psql commands in PostgreSQL).
- The parser walks the code tokens and gives each one its kind (keyword, identifier, function name, type, word, literal) because the case options act per class. It opens the nodes of `grammars/sql.grammar` (named after sqlfluff's segment types; `*_statement` without a node of its own is a `Statement`) and puts the zero-width `Indent`, `ImplicitIndent` and `Dedent` the layout reads. Queries, DML (insert, update, delete, merge, `RETURNING`, T-SQL `OUTPUT`), expressions, types, table functions and the statements of T-SQL's procedural blocks have their own functions. Other statements (most DDL, `GRANT`, `SET`, `COPY`, ...) go through one generic walker (`generic.rs`) that names their words by position, with the per-statement keyword rows in `parse/data/statements.txt`.
- `tree.rs` replays the flat result into the lossless tree (trivia attached by the shared rule, a zero-width `Eof` ends the root).
- A statement that is not understood to its end becomes an `ERROR` node up to the next `;` and a parse error at its span; the rest of the file still parses. Nothing panics (every loop consumes a token or ends; a bracket nesting over `MAX_DEPTH` is taken as flat tokens), so there is no file size limit and no `catch_unwind`.

Why not another parser (measured on sqlfluff's dialect fixtures, 877 files, and the PostgreSQL regression inputs): sqruff (the former parser) failed 8/214 ansi, 62/210 postgres, 36/258 mysql, 38/195 tsql files and 185/244 regression files, parsed 0.4 MB/s with 650 bytes of memory per input byte and could panic; sqlparser-rs failed 33, 114, 158, 146 files, aborts at the first error and has no concrete tree; tree-sitter-sequel failed 79 to 214 files; tree-sitter-postgres is PostgreSQL only and 15 MB. One parser that is lossless and recovers per statement, with the dialects as modes and data, covers more and is faster.

Measured (`cargo test --release -p lang-sql --test integration -- --ignored --nocapture conformance::scaling`, `conformance::sqlfluff_fixtures`, `conformance::postgres_regress`; Graviton arm64, one thread): parse failures on the fixtures ansi 3/214, postgres 6/210, mysql 8/258, tsql 9/195 and 56/244 regression files (those include statements that must fail); about 19 MB/s on `bench/inputs/sql` (sqruff: 0.4); peak process memory 13 MB at 489 KB and 73 MB at 4 MB of input (sqruff: 261 MB at 489 KB); no cost for a dialect's first file. Stripped release `sudsy`: 49.52 MB (`scripts/binsize.sh`), 4.33 MB less than with sqruff linked (53.85 MB, x86-64 Linux).

## Options
<!-- generated:options -->
| Option | Values | Default | Description |
|---|---|---|---|
| `dialect` | `ansi`, `postgres`, `mysql`, `tsql` | `ansi` | The SQL dialect the file is parsed as. |
| `line_width` | int 20-1000 | `80` | Preferred maximum line width in columns. |
| `indent_style` | `space`, `tab` | `space` | Indent with spaces or tabs. |
| `indent_size` | int 1-16 | `4` | Columns per indentation level (also the width of a tab). |
| `final_newline` | bool | `true` | End the file with a newline. |
| `end_of_line` | `auto`, `lf`, `crlf` | `auto` | Line break style of the output: `lf`, `crlf`, or `auto` to keep the file's first line break. |
| `comma_position` | `trailing`, `leading` | `trailing` | Where a comma goes when a list is broken over lines: at the end of a line, or at the start of the next one. |
| `keyword_case` | `consistent`, `upper`, `lower`, `capitalise` | `consistent` | Case of keywords: `consistent` follows the first one written (all that follow are brought to its case once one differs), the others set it. |
| `identifier_case` | `consistent`, `upper`, `lower`, `capitalise` | `consistent` | Case of unquoted identifiers: `consistent` follows the first one written (all that follow are brought to its case once one differs), the others set it. |
| `function_case` | `consistent`, `upper`, `lower`, `capitalise` | `consistent` | Case of function names: `consistent` follows the first one written (all that follow are brought to its case once one differs), the others set it. |
| `literal_case` | `consistent`, `upper`, `lower`, `capitalise` | `consistent` | Case of `NULL`, `TRUE` and `FALSE`: `consistent` follows the first one written (all that follow are brought to its case once one differs), the others set it. |
| `type_case` | `consistent`, `upper`, `lower`, `capitalise` | `consistent` | Case of data types: `consistent` follows the first one written (all that follow are brought to its case once one differs), the others set it. |
<!-- /generated -->

## Formatting
Layout is Sudsy's own code (`src/layout/`) on the parse tree, following sqlfluff's layout rules (LT01 to LT15) with its defaults ([the catalog](../parity/sqlfluff.yaml)). The tree is flattened to blocks (tokens) and the points between them (spacing, line breaks, indentation); `space.rs` sets the spacing (LT01, LT06, LT07, LT10, LT11 and so on), `breaks.rs` the line breaks (LT08, LT09, LT12, LT13, LT14, LT15), `indent.rs` the indentation (LT02, LT03). The rules run in turn until the layout stops changing, as `sqlfluff fix` does, and the result is printed line by line. `wrap.rs` breaks lines over `line_width` (LT05: the outermost indent points, or the operators and commas by precedence, as sqlfluff picks them), the `[case]` rows of `format.rules` (`core_format::case`) set the case of words (CP01 to CP05), `blocks.rs` adds the indents that the MySQL (procedural blocks) and T-SQL (`IF`, `ELSE`, `WHILE` bodies) parses leave out. `comma_position` (LT04) puts the commas of a broken list at the end of the line (default) or the start of the next one. Not yet written: comment placement; those parts stay as written.

### Case
Five classes of words have a case option: keywords and binary operators (`keyword_case`), unquoted identifiers (`identifier_case`), function names including `CURRENT_DATE` and the like (`function_case`), `NULL`/`TRUE`/`FALSE` (`literal_case`) and data types (`type_case`). `upper`, `lower` and `capitalise` set it. `consistent` (the default, sqlfluff's) reads the words of a class in order and keeps the cases the words so far agree on (upper, lower, capitalised, in that preference); the word that rules out the last one, and every one after it, is set to the last case that was still possible, so the whole file ends in one case. The parser's word types decide the class (`parse/`): a keyword inside a data type is a type, the last part of a qualified function name (a user's function) and datatype methods are no class and stay as written. The safety check compares these words lowercase.

## Lint
Rules of sqlfluff's aliasing (AL), ambiguous (AM), convention (CV) and structure (ST) families that catch bugs or dead code are on by default (`null-comparison`, `distinct-group-by`, `unused-cte`, `duplicate-table-alias`, `self-alias`, `union-distinct`, `else-null`); the style ones (`implicit-alias`, `count-star`, `prefer-coalesce`, `right-join`) are off. Most are pattern rules over the node kinds in `crates/lang-sql/rules/`; the ones that look at names are Rust (`src/rules.rs`). Layout (LT) and capitalisation (CP) are the formatter's, so they have no rules. Names are compared lowercase without quotes; no rule resolves columns or schemas, so sqlfluff's reference (RF) rules are planned ([the catalog](../parity/sqlfluff.yaml)).

## Parity
`scripts/parity.sh format sqlfluff` (sqlfluff 4.5.0 `fix --rules layout,capitalisation` against `sudsy format`, one dialect per fixture): sqlfluff's dialect fixtures and PostgreSQL's regression inputs of the conformance corpus. Identical to sqlfluff's on 678 of the 867 files sqlfluff accepts (ansi 203, postgres 122, mysql 208, tsql 108, PostgreSQL regress 37; 1,121 files, 254 of them refused by sqlfluff itself: psql commands, template tags); a second format changes none. Every difference is a named gap in `tests/parity/sqlfluff/differences.yaml`: files our parser does not accept (`parse-gap`), and the layout not written yet (`layout-gap`).

## Conformance
`bash tests/conformance/fetch-sql-corpus.sh`, then `cargo test --profile corpus -p lang-sql --test integration -- --ignored --nocapture conformance::`: sqlfluff's dialect fixtures (ansi, postgres, mysql, tsql) and PostgreSQL's regression inputs; every file prints back byte for byte and satisfies the trivia rule, and the files with parse errors are counted per dialect. `conformance::format_is_stable` formats every file that parses twice (stable output, safety check passed).
