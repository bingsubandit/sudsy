# Design: Ruby format, what is open

The format leg is at the usability bar (86.0% of the files equal to RuboCop's Layout cops, second format 0; how it works: [../formatting/ruby.md](../formatting/ruby.md)). This note holds the gaps that are left; it goes when they are closed.

The 2,648 files that still differ, by the cops that change them (the first line that differs, over the files that differ in indentation only, and the upper bounds of the whole leg):
- Alignment to a column that the layout does not decide alone: later arguments and hash pairs under the first one on its line (`ArgumentAlignment`, `HashAlignment`, `ParameterAlignment`), a continuation under the first operand of an assignment, a chain root that starts its own line. About 700 files change in indentation only.
- Token spacing (`Layout/Space*`, 17,153 upper bound), comment text and trailing whitespace (18,001), line length (18,651) and brace layout (18,958; these two break lines).
- `<<-` and `<<` to `<<~` with the body re-indented: 33 files. The Prism tree check refuses it (the opener text differs and a `String` becomes an `InterpolatedString`), so the check has to learn that rewrite before the formatter can make it.
- 32 files that RuboCop leaves as they are change: blocks and chains it leaves where the source has them, aligned blanks inside a hash.
