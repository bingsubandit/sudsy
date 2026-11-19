# C/C++ formatter plan

The C/C++ formatter is built on the shared Doc printer and the shared shapes (`core_format::shapes`), like every other formatter (user decision 2026-11-19: one layout engine). Reference: clang-format 20.1.8, measured by the clang-format parity runner.

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
Write the mapping table of [../architecture/adding-a-language.md](../architecture/adding-a-language.md): each clang-format construct to a shape and the options it needs. Expected rows: braces and statements to `shapes::block` and `shapes::compound`; control statements to `shapes::flow`; argument and parameter lists to `shapes::call_arguments` and `shapes::list` (`BinPackArguments`/`BinPackParameters` as options; packing through `fill`); binary expressions to `shapes::binary_chain` (`BreakBeforeBinaryOperators` as `binary_operator_line`); `=` to `shapes::assignment`; return type on its own line to `shapes::signature` (an option); base clauses to `shapes::heritage`; access specifiers and `case` labels to `shapes::block`/`shapes::declaration` (an option for the label offset); preprocessor lines through `core_format::directives` (kept as written); comments through `core_format::comments`; trailing comment alignment through `columns`/`align`. A construct with no shape extends the nearest shape with an option, in core-format, never in `lang-cpp`.

**Checkpoint 2:** report the table. The lead approves it before code is written.

Then build. Target: the `lang-cpp` formatter is glue and hooks, under 800 hand-written lines; the HARD RULE review of `scripts/land.sh` passes without `Lead-approved` lines for layout.

### Phase 3: measure, rank, fix in shared code
Run the parity, classify the differing files by cause, rank by count. Known shared defect from the first attempt: `conditional_group` measures a candidate only up to its first possible break, so it picks an aligned layout although a later element does not fit (part of the 3,333-file argument-packing group). Fix it in `core_format::printer`; every language's format ratchet must stay at or above its minimum. Count what remains that only a cost-based choice explains (bin-packing row counts, break after `=`, return-type break); that count decides whether a cost choice is added to the Doc printer (lead and user decision, not part of this plan).

### Phase 4: speed
cpp/c format instructions and allocations per byte within 2x of `java.format` on the bench inputs (the first attempt ran 1,034 M instructions and about 20 allocations per byte for 97 KB). Check [../performance/checklist.md](../performance/checklist.md).
C/C++ parse per byte within 1.5x of `java.parse` (deep pass; now 1.86x for C and 1.96x for C++).

## Done when
second-format 0 and refused 0 in every project; identical counts at least the verbatim baseline in every project and recorded per project; the formatter on shapes with the HARD RULE review passing; docs (`docs/ai/formatting/cpp.md`, roadmap status table, `docs/languages.md`, numbers) current. Landings: phase 0, then phases 1-2 (tree and first formatter), then each phase 3 fix.
