# C/C++ formatter plan

The C/C++ formatter is built on the shared Doc printer and the shared shapes (`core_format::shapes`), like every other formatter (user decision 2026-11-19: one layout engine). Reference: clang-format 20.1.8, measured by the clang-format parity runner.

## What the parity number compares

Line wrapping is the one thing Sudsy deliberately does differently from clang-format, so the clang-format run does not compare it: where a statement, a list or an enum breaks and the indent of the continuation lines. Both outputs are read through `sudsy debug tree`; a file is equal when the tokens are equal and every gap between two tokens is equal, except at a wrap point (a gap inside one statement, declaration or directive), where a newline equals a space and the indent is free; an indent after a wrap point that differs in one output only is free too when Sudsy, run with `wrap_lines: false` on the reference output, gives the reference's indent there (the wrap choice alone explains it). Everything else is exact: blank lines, brace placement, spacing, comments, statements of a macro body (a gap after `;`, `{` or `}` outside parentheses, read from the token tree: no body parser) and text under `clang-format off`. The ratchets count these files; byte-identical files and files with more over-long lines than clang-format are printed, not judged. The comparison is the `wrap_compare` of the `clang-format` entry (`scripts/parity_wrap.py`, [parity.md](../development/parity.md)). Checked with `PARITY_CONTROLS=1`: clang-format against itself 6,639 of 6,639 equal; wrap-only damage (all wraps joined, all split, random) 6,639 of 6,639 equal; another brace style 15.2%, another pointer style 20.6%, 5% of blank lines dropped 36%; a changed byte in the first string literal of the reference differs in every file that has a literal (equal 3,357: the files without one), and a literal split in two by hand differs except where the tool split a literal itself (equal 3,406). A string literal that clang-format split at the column limit (`BreakStringLiterals`) compares as the one literal: the runs of adjacent literals of both outputs are equal in order, prefix and joined contents, outside a directive and outside `clang-format off`, and the tool's count of literals in a run is not the source's (a source of two literals that Sudsy joined is a difference). Measured: 3,040 of 6,639 equal (45.8%), 1,526 byte-identical. `sudsy format` changes 4,766 of 6,639 files (spacing of keywords, assignments and operators in 3,536, indentation in 2,542, line breaks added or joined in 2,509, blank lines over `max_blank_lines` in 835, trailing blanks in 370) and changes no token of its input except trailing blanks of a `//` comment in 2 files. Printed as written: `*`, `&` and `&&` spacing where the tree shows no declarator or where an operation may be a declaration the parse took for one (`T * x`, `(T)&x`), source line breaks and wraps, casts, opaque lines (statement macros, directive bodies), column alignment. The 3,599 differing files by first cause of the comparison: spacing 2,021, brace placement 673, comment gap 343, indent 244, statement join 162, tokens 80, blank lines 53, `clang-format off` 22, wrap blank 1; spacing is the only cause in 1,095 files. The names of `differences.yaml` (`files:`) classify these files by their first differing gap (a script reads both outputs; 60 files are not classified). A full run takes about 4 minutes on 8 cores (about 9 with the controls). The bar is 85% of these 6,639 files on this comparison, and a split string literal counts as wrapping.

## Why the tree comes first

The shapes read role-tagged nodes through `Grammar::slot` (`#call` with `callee`/`arguments`, `#binary-chain` with `left`/`operator`/`right`, `#assignment`, `#block`, signatures, ...; [../architecture/syntax-tree.md](../architecture/syntax-tree.md), [../formatting/engine.md](../formatting/engine.md)). The C/C++ tree of `crates/lang-cpp/src/structure.rs` has only unwrapped lines, bracket groups and directives, so no shape can apply. A formatter written over that tree has to decide every layout itself, which is a second layout engine in the language crate (rejected: branch `agent-ae9bc9412e18afccc`, 0457095c, 66 HARD RULE findings, 4,000 lines).

Building the tree is parsing, which is language code (as the SQL and Java parsers are): the parser decides which tokens form a call, a cast, a template argument list, a declaration. Layout is not: every layout decision goes through a shape.

## Phases

Each phase ends with a measurement in `.cargo/tmp/TASK.md`. Stop and report to the lead at the two checkpoints.

### Phase 0: salvage and baseline (one landing)
From 0457095c take only: the clang-format parity runner (`scripts/parity_tools.json` entry, `tests/parity/clang-format/`, `docs/ai/parity/clang-format.yaml`), the `lang.yaml` options and presets clang-format needs, the generated schema, and the cause classifier (as a generic script under `scripts/` if it is not C++-specific, else left out). Leave out `format/` (model, annotate, wrap, print, verify) and the three Doc primitives unless a shape needs them later. The formatter stays verbatim. Measure the verbatim baseline per project and set the ratchets to it. Land this alone.

### Phase 1: a tree the shapes can read
Deepen the parse in `lang-cpp` (inside each unwrapped line and bracket group), with node kinds and roles in `grammars/cpp.grammar` modeled on `grammars/java.grammar` and `grammars/csharp.grammar`:
- `precedence` lines for the binary operators (as C# has); binary expressions as `#binary-chain` nodes;
- calls (`#call`: `callee`, `arguments`), member access (`#member-access`), index, unary, casts, conditional `?:`;
- assignment and initializers (`#assignment`), braced initializer lists (list nodes);
- declarations: function declarations and definitions with parameter lists (what `shapes::signature` reads), class/struct/enum bodies, base clauses (`shapes::heritage`), access specifiers, namespaces, templates;
- statements: `if`/`else`, loops, `switch`/`case`, `return`, with the roles `shapes::flow` and `shapes::block` read.

Ambiguities (`<` as template or comparison, `*`/`&` as pointer or operator, declaration or expression) are resolved the way clang-format does, without preprocessing. This is the part of 0457095c's `annotate.rs` that classified tokens; its penalties and layout tiers are not reused. If an ambiguity needs checkpoint and rollback in the parser, stop: that is deferred row P7b of [unification.md](unification.md), which runs first.
Checks: lossless round-trip on all corpora, `cargo xtask codegen --check` (rows validated against the grammar), cpp/c parse bench within +30%.

**Checkpoint 1:** report the node and role list, parse speed and any ambiguity left as an opaque node, before building the formatter on it.

### Phase 2: mapping table, then the formatter on shapes
Mapping table (approved). Plain options (`brace_wrapping`, `short_*`, `pointer_alignment`, `space_before_parens`) are `lang.yaml` options read in shared Rust (shapes, `Flow::want`, spacing code); no option conditions in `[space]` rows and no new row keys.

| clang-format construct | Shape and option |
|---|---|
| blank lines | `max_blank_lines`, `[blank]` rows |
| braces | `brace_wrapping` words via `brace_on_new_line` |
| if / loops / try | `shapes::flow`, `short_ifs`, `short_loops`; new `compound::body` join policy |
| switch / case | `indent_case_labels`, `short_case_labels`; case label outdent (member outdent) |
| access specifiers | `access_modifier_outdent`; new member outdent in `declaration.rs` |
| namespaces | `namespace_indentation`; `inner` = `[layout]` row, parent NamespaceDeclaration |
| class / enum bodies | `short_enums`; delimited list Fit |
| base clauses | `shapes::heritage`; new option: before colon, after colon, before comma |
| constructor initializers | parser: Initializer children + `#heritage` |
| signature / parameters | Fill; new column-aligned Doc primitive; `break_after_return_type`; `penalty_return_type` counted in Phase 3 |
| call arguments | parser: Arguments delimited list with `#separator`; Fill; `hug_last_argument` |
| binary / ternary | new `BinaryPolicy` `non_assignment`; `AlignOperands` via column-align |
| `=` | parser: `declarator` / `initializer` fields on SimpleDeclaration |
| init lists | row InitList : array, `ListStyle.padding`, `columns.rs` |
| lambdas | `short_lambdas` via `join` |
| pointer alignment, spaces | plain options in shared Rust |
| templates | parser: `declaration` field on TemplateDeclaration |
| comments | new `align.rs` run rule over `#statement` / `#member` |
| preprocessor | verbatim layout + `continuation.rs`, `align_escaped_newlines`; `macro_word` / `macro_name`, `StatementMacros` / `ForEachMacros` as plain `lang.yaml` word lists read by the shared role pass |
| includes | not built |
| wrapping | Phase 3 |
| unread constructs | flow shape |

Shared capabilities, built in the shared crates with unit tests or a fixture of a second language: (1) column-aligned Doc primitive in `core-format::printer` (run the format parity of csharpier, prettier, prettier-js, ruff, gjf, ktlint, php-cs-fixer before landing); (2) plain options in `Flow::want`; (3) `compound::body` join; (4) heritage colon/comma option incl. constructor initializers; (5) member outdent (case labels, access specifiers); (6) `align.rs` over statements; (7) none: assignments are `#assignment` nodes, so `binary_operator_line: start` already is clang-format's `NonAssignment`; (5) is a column outdent in `core-format` (shift by N columns, negative allowed, read from `access_modifier_outdent`), built in the first landing of the formatter step. Directive-aware shape checking and `macro_word` / `macro_name` are Phase 2 work.

**Checkpoint 2:** report the table. The lead approves it before code is written.

Then build. Target: the `lang-cpp` formatter is glue and hooks, under 800 hand-written lines; the HARD RULE review of `scripts/land.sh` passes without `Lead-approved` lines for layout.

### Phase 3: measure, rank, fix in shared code
Run the parity, classify the differing files by cause, rank by count. Known shared defect from the first attempt: `conditional_group` measures a candidate only up to its first possible break, so it picks an aligned layout although a later element does not fit (part of the 3,333-file argument-packing group). Fix it in `core_format::printer`; every language's format ratchet must stay at or above its minimum. Count what remains that only a cost-based choice explains (bin-packing row counts, break after `=`, return-type break); that count decides whether a cost choice is added to the Doc printer (lead and user decision, not part of this plan).

### Phase 4: speed
cpp/c format instructions and allocations per byte within 2x of `java.format` on the bench inputs (the first attempt ran 1,034 M instructions and about 20 allocations per byte for 97 KB). Check [../performance/checklist.md](../performance/checklist.md).
C/C++ parse per byte within 1.5x of `java.parse` (deep pass; now 1.86x for C and 1.96x for C++).

## Done when
second-format 0 and refused 0 in every project; identical counts at least the verbatim baseline in every project and recorded per project; the formatter on shapes with the HARD RULE review passing; docs (`docs/ai/formatting/cpp.md`, roadmap status table, `docs/languages.md`, numbers) current. Landings: phase 0, then phases 1-2 (tree and first formatter), then each phase 3 fix.
