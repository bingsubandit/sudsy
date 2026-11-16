# SQL (`lang-sql`)

## Parser
[sqruff](https://github.com/quarylabs/sqruff)'s `sqruff-lib-core` and `sqruff-lib-dialects` 0.44.0 (Apache-2.0), pinned to the exact version in the root `Cargo.toml` (dependabot ignores them; a bump is a deliberate change of `grammars/sql.grammar` and `crates/lang-sql/src/adapter.rs`, followed by the conformance run). One language `sql` covers `*.sql`; the dialect is the option `dialect`, one of the four offered: `ansi` (default), `postgres`, `mysql`, `tsql` (the fixer `sqruff-lib` depends on `sqruff-lib-dialects` with its default features, so every dialect of sqruff is compiled in; the other ones are not offered because they hang or panic). The dialect travels in `SourceFile::language_version`; a dialect is built on a thread's first file of it.

sqruff lexes and parses a whole file into segments (raw leaves and nodes named by sqlfluff's segment types). `adapter.rs` builds our tree from them:
- the leaves in order are the tokens (`Whitespace`, `Newline`, `LineComment`, `BlockComment`; `Keyword`, `Identifier`, `QuotedIdentifier`, `String`, `Number`, `Operator`, and the punctuation `( ) , ; .`); zero-width leaves (`Indent`, `Dedent`, `EndOfFile`) are dropped, a leaf that does not equal the text at its place fails the file;
- a segment type is a node when `grammars/sql.grammar` has a node of its name in Pascal case (`select_statement` is `SelectStatement`); any other `*_statement` is a `Statement`; the other types are transparent (their leaves stay, the node does not). `Unparsable` is an `ERROR` node and a parse error at its span;
- trivia are attached by the shared rule, whatever sqruff's tree says (`core_syntax::trivia::attach`), and a zero-width `Eof` token ends the root.

Failure modes, all leaving the file unchanged (it is one `Unknown` token and one parse error, which the formatter and the linter refuse):
- sqruff panics on some inputs (an `unimplemented!` or `unwrap` in a dialect): the parse runs under `catch_unwind` and the error says the parser failed on the file;
- a file over `MAX_BYTES` (512 KB) is not parsed: sqruff parses 0.3 to 1 MB/s, so the limit keeps one file to about 1.6 s (peak memory is about 650 bytes per input byte);
- a nesting deeper than sqruff's `DEFAULT_MAX_PARSE_DEPTH` (600) is sqruff's own error.
Dialects whose parser hangs (`soql`) or panics often (`redshift`, `sqlite`, `starrocks`, `trino`) are not offered.

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
<!-- /generated -->

## Formatting
sqruff is the parser only: layout is Sudsy's own code (`src/layout/`) on the parse tree, following sqlfluff's layout rules (LT01 to LT15) with its defaults ([the catalog](../parity/sqlfluff.yaml)). The tree is flattened to blocks (tokens) and the points between them (spacing, line breaks, indentation); `space.rs` sets the spacing (LT01, LT06, LT07, LT10, LT11 and so on), `breaks.rs` the line breaks (LT08, LT09, LT12, LT13, LT14, LT15), `indent.rs` the indentation (LT02, LT03). The rules run in turn until the layout stops changing, as `sqlfluff fix` does, and the result is printed line by line. `wrap.rs` breaks lines over `line_width` (LT05), `blocks.rs` adds the indents of MySQL procedural blocks that sqruff parses flat. Not yet written: capitalisation (`*_case` options), `comma_position`, comment placement; those parts stay as written.

## Parity
`scripts/parity.sh format sqlfluff` (sqlfluff 4.5.0 `fix --rules layout,capitalisation` against `sudsy format`, one dialect per fixture): sqlfluff's dialect fixtures and PostgreSQL's regression inputs of the conformance corpus. Identical to sqlfluff's on 472 of the 867 files sqlfluff accepts (ansi 166, postgres 88, mysql 167, tsql 44, PostgreSQL regress 7; 1,121 files, 254 of them refused by sqlfluff itself: psql commands, template tags); a second format changes none. Every difference is a named gap in `tests/parity/sqlfluff/differences.yaml`: files our parser does not accept (`parse-gap`), and the layout and capitalisation not written yet (`layout-gap`, `case-gap`).

## Conformance
`bash tests/conformance/fetch-sql-corpus.sh`, then `cargo test --profile corpus -p lang-sql --test integration -- --ignored --nocapture conformance::`: sqlfluff's dialect fixtures (ansi, postgres, mysql, tsql) and PostgreSQL's regression inputs; every file prints back byte for byte and satisfies the trivia rule, and the files with parse errors and panics are counted per dialect. `conformance::format_is_stable` formats every file that parses twice (stable output, safety check passed).
