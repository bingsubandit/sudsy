# C/C++ format: plan to 85%

Reference: clang-format 20.1.8, 6,639 files. Equal now: 4,827 (72.7%) under the comparison with short bodies as wraps
(4,668 under the old one: +159 comes from the comparison, none from the formatter). 85% is 5,644 (817 more).

The plan has two contents: differences the comparison should not count (A), and the class-a work that takes the rest to
85% (B). The classification covers every gap record of the 1,812 differing files (73,252 records: 53,916 a, 7,462 b,
11,874 c).

## Classes

| Class | Meaning | Files with it | Sole-class files |
|-------|---------|--------------:|-----------------:|
| a | real: spacing, pointer side, token changes, blank lines, comment places, a wrap-independent indent, an option not implemented | 1,500 | 903 |
| b | a line's indent, a comment line's indent or a brace gap that follows a wrap point which has a newline in one output only | 704 | 171 |
| c | column padding: a same-line gap of a tab or two or more blanks (trailing comments, tables, `\` of macros) | 454 | 88 |

Mixed sets: a+b 284, a+b+c 196, a+c 117, b+c 53. Reachable with only class a fixed and the comparison as it is: 5,730
(86.3%); with a fixed and class b not counted: 6,185 (93.2%); a fixed and class c not counted: 5,935 (89.4%); all three
not counted but a: 6,639. With nothing fixed: class b not counted 4,998 (75.3%), class c not counted 4,915, both 5,139
(77.4%).

## A. What the comparison should not count

Both are proposals (nothing changed): the records carry the flag `w` (`wrapped_before` in `scripts/parity_wrap.py`) that
decides b.

- Class b, 704 files (171 sole). Counted by three exact rules of `compare`: the brace gaps, the lines next to comments
  and the `indent` branch (the line's indent relative to the anchor line of its container). The indent of such a line
  is relative to the anchor line, and the anchor line's own continuation lines depend on where the statement wrapped.
  Extension of "wrapping is ignored": a gap with the same newline count (or a brace gap) is free when a wrap point
  between the gap's anchor token (else the start of its statement) and the gap has a newline in one output only. Where
  the wraps are equal up to the gap the difference stays exact. Typical: `; return` in `ReturnStatement`, `) {` after a
  wrapped signature, comments in a parameter list whose first parameter moved.
- Class c, 454 files (88 sole). Counted by the exact `spacing` and `comment-gap` rules of a gap without a newline.
  Extension: a gap without a newline whose blanks are a tab or two or more blanks on either side is free (padding to a
  column, decided by the width of the lines around it). It also hides a run Sudsy does not align at all; the pairs of
  class c in the report show that most are `, 714` in tables and trailing comments.

## B. Class-a work

Each mechanism is a set of gap records mapped by cause, token kinds and parent kinds. A file is won when all its
mechanisms are fixed (the greedy cover counts it at that step). Size: rows (R: a few kit rows or lang.yaml kind sets),
S (a shared mechanism of 50 to 150 lines plus rows), D (a decision in the deep parse of 200 or more lines: the tree has
to tell a declaration from an expression, a template from a comparison). Sizes are estimates, replaced by measurements
when the mechanism is built. Measured earlier: R work converts nearly all its files, D work 10 to 25 percent until the
decision is complete.

| Mechanism | Files | Sole | Size | What |
|-----------|------:|-----:|------|------|
| pointer | 229 | 130 | D | `*`, `&` of a declarator in `ParenGroup` (`for (T *p`), `Assignment`, `Binary`, `Condition`, directives, `Line`: the deep parse decides declaration against product, cast against subtraction |
| spacing-other | 170 | 88 | R | `->` then a name in `QualifiedName`, a string next to a macro name, `...`, `::` after a string (about 15 rows) |
| paren-space | 103 | 47 | R | the space before `(` of calls and parameter lists (gnu, webkit; `SpaceBeforeParens` per construct kind) |
| cmt-indent | 118 | 57 | S+D | comments inside lists (hug list, branch `cpp-hug-list`: 50 won, 16 lost; the losses need the `Dot|InitList`, `Call` and `Binary` comment indents) |
| statement-join | 76 | 41 | S | a loop-macro body stays on its line, a statement macro split, a directive's arguments |
| comment-outside | 61 | 38 | S | a comment before `;` of an empty statement, before `}` (blank lines), after `{` |
| brace-placement | 71 | 34 | R+S | `) {` and `{` per construct (BraceWrapping words), empty bodies after a wrapped initializer |
| binary-space | 59 | 29 | D | `-` as binary or unary after `)`, `|` in a directive, `=` before `::` |
| angle | 51 | 16 | D | `<` as template or comparison (`template <`, `a < b`), `> (` after a template |
| comment-trailing | 46 | 27 | S | a trailing comment after `{` or `,` of a list |
| short-body | 47 | 18 | S | empty bodies `{}` against `{ }`, a body after a wrapped initializer |
| indent-cont | 78 | 24 | S | the indent of a label's block, a loop macro's body, macro argument items |
| indent-brace | 64 | 9 | S | `}` and `{` lines whose anchor is a label or a macro head |
| blank-lines | 24 | 15 | R | a blank before `}`, after `{`, around access specifiers |
| bitfield-space | 22 | 17 | R | the colon of a bitfield (`a : 3`) |
| braced-pad | 22 | 4 | R | the blanks inside `{ }` of initializer lists (Cpp11BracedListStyle) |
| comment-in-list, tokens-reorder (sorted `using`), brace-other, tokens, cast-space, indent-stmt, comma-space, format-off | 18 to 1 each | | R and S | the tail, 90 files |

Joint cover (greedy, each step the mechanism that completes most files; 100% yield assumed):

- With the comparison as it is: files with a b or c record cannot be won by class-a work, so 903 files are left. Order:
  pointer 130, spacing-other 97, paren-space 70, cmt-indent 68, statement-join 53, comment-outside 48, brace-placement
  45, binary-space 40, angle 40, comment-trailing 42, short-body 36, indent-cont 32, indent-brace 54, blank-lines 22,
  bitfield-space 21, braced-pad 20: 5,645 (85.0%) after 16 mechanisms. The whole of class a: 5,730 (86.3%).
- With the class-b extension: 1,358 files are open. Order: pointer 152, spacing-other 121, cmt-indent 95, paren-space
  85, statement-join 67, brace-placement 58, comment-outside 58, angle 48, binary-space 53, comment-trailing 50,
  short-body 45: 5,659 (85.2%) after 11 mechanisms; the rest (indent-brace, indent-cont, blank-lines, ...) reach 6,014
  (90.6%).
- With the class-b and class-c extensions: 1,812 files open, 5,690 (85.7%) after pointer, cmt-indent, spacing-other,
  paren-space, statement-join, brace-placement, comment-outside, comment-trailing (8 mechanisms); all of class a gives
  6,327 (95.3%).

Order of the build: R items and kind sets first (they convert almost fully), then the S items, then the D items
(pointer, binary-space, angle) whose tree decision is the work. After the OK the set is built in one landing.

## Measured so far

- Comments outside lists, first batch: a region under `clang-format off` keeps the comments after the marker and the
  indentation of its first line (`FormatCtx::verbatim_region`); a comment at column 0 right above a directive stays at
  column 0. 4,597 to 4,606.
- Option cases and kind sets: a conversion operator with a ref-qualifier is a function, `noexcept`, `override`, `final`
  and `requires` behind a `&` are no names, a `name(args) { }` macro block joins like a function body unless it is a
  loop macro, a body that holds a block of its own or has a directive in its signature stays expanded, `Line`,
  directives and function-pointer groups hold declarator marks, `a[i-1]` gets blanks. 4,607 to 4,668.
- List hug (comments in lists): 4,607 to 4,641 (50 won, 16 lost). Not landed, branch `cpp-hug-list`.
- Comparison: short bodies as wraps where the style's option permits the join. 4,668 to 4,827 (+159).
