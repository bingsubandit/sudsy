# Pay only for what you use: plan

Problem: a shared formatter feature or a batch of rules for one language makes other languages slower. Today's cases: shell comment alignment (printer, every language +0.5..1%), C/C++ layout options (php.format +29.7%, toml +7%), YAML comment ownership (github-actions +3.9%), shell rules (json/toml/graphql lint +1.2%, github-actions lint-query +4.7%). Each cost a rejected finish (20 min CI) plus a fix round.

## What the code does today (origin/main 69ad8611)

The causes are not missing on/off switches. They are lookups and loops whose cost does not depend on whether a language uses the feature.

| Area | Today | Why an unused feature still costs |
|---|---|---|
| Options | `ResolvedOptions(BTreeMap<String, OptionValue>)` (core-config/schema.rs:277), read by name at the use site | `cx.options.get("align_comments")` is a map lookup per node even when the option is absent. `strings()` / `string_lists()` clone a `Vec<String>` per call; blank.rs `by_options` does it per neighbour pair. `BinaryPolicy::of`, `ChainPolicy::of`, `AssignPolicy::of` (3-8 lookups each) are rebuilt per node in 15+ call sites. |
| Grammar tags | 30 hot tags in a bitmask, every other tag a linear scan (core-syntax grammar/mod.rs:89, 273) | A new tag check in shared code is a string scan per node for every language. 135 `has_tag(` calls in core-format. |
| Rule selection | `activate` loops over every rule on every lint call, not cached (core-rules/configure.rs:15; core-incremental/rules.rs:93) | Grows with the total rule count, for every file. |
| Rule dispatch | `check_body` runs for every active rule at every body node (core-rules/engine.rs:143) | One more active rule adds a call per node, plus a `RuleContext::new`. |
| Embedded code | Always parsed in `lint_body`, whatever rules are on (core-incremental/rules.rs:214) | Dockerfile, Markdown, GitHub Actions pay for embedded shell even with every shell rule off. |
| Analyses | Already on demand: rules declare `uses: &[Layer]`, the scope model is built by the first scope rule and cached per file | Works. Keep it; the plan only reuses it. |
| Loading | Grammars, `format.rules` and pattern YAML are already lazy (`OnceLock`) per language; off rules never compile their patterns | Works. More lazy loading would not fix the cases above. |

So "lazy loading" and "per-language enable flags" both miss the actual causes. The fix is to make an unused feature cost nothing *by construction*, through the levers that already exist.

## The idea, and the alternative it replaces

Rejected: a per-language feature switch (`features: { comment_align: false }`, a cargo feature per language, or a `Features` bitset that each new shared feature must add itself to).
- It is a second source of truth next to the option that already turns the feature on. An agent who finds the switch off for Java cannot tell whether the feature is missing or only switched off. This is the reimplementation risk you raise.
- Every new shared feature then needs a switch, a default and a test for both states: more work per language and per rule, which is your first concern.
- It still leaves the per-node lookup cost when the switch is read by name.

Proposed: no new switches. A feature stays on exactly when the language's data says so (an option set in `lang.yaml`, a kind set, a grammar tag, a rule in the preset), as today. What changes is the cost of asking:
1. Options are read by a numeric id from a vector resolved once per file. An absent option is one array read. List values are borrowed slices, never cloned. Policy structs are built once per file and cached.
2. Every grammar tag is a bit in a per-kind bitset, not only 30 hot ones.
3. The active rule set is computed once per config and cached. Rules are dispatched by node kind, so a rule only runs at the kinds it visits.
4. Embedded code is parsed only when an active rule or the formatter needs it.

## Your two concerns

### Does this make adding languages or rules harder?

Mostly no, with one real cost:

| Change | Effect on a new language or rule |
|---|---|
| Options by id | No change for language authors: `lang.yaml` options are already codegen'd into name constants; they become id constants. Shared code writing a new option adds one line to a shared option list (the id must exist before shared code can name it). That is new: today shared code can read any string. |
| Tag bitsets | None. Tags are declared in the grammar as today; codegen assigns the bit. |
| Rule set cache | None. |
| Kind dispatch | **Real cost:** a Rust rule declares the kinds it visits (pattern rules already know theirs from the pattern). A rule that declares none keeps today's behaviour (runs everywhere, slower), so nothing breaks; `cargo xtask new-rule --rust` writes the declaration into the starter. |
| Embedded parse on demand | None for authors. |

The current friction is worse: each overrun costs a 20-minute CI round and a lead ruling, and agents work around it with ad hoc early exits (`if !cx.options.flag(..)`). Those exits are themselves string lookups, and they are easy to forget.

### Will a later agent know the feature just needs turning on?

This is the larger risk, and it exists today with or without this plan. Comment alignment, heredoc re-indent and raw-string re-indent each landed this week as shared modules with one user. An agent later asked to add comment alignment to Java must find `CommentStyle::column_option` and set `comment_column: align` in Java's `lang.yaml`.

What exists: `docs/ai/kit/index.md` (generated), one row per shared concept (`id | module | what | tools | used by`); adding-a-language.md requires a mapping table that searches it by tool and behaviour words; `cargo xtask dupes --check` catches copied code. What is missing: the index does not say *how* a language turns a concept on.

The plan adds:
1. Index column "turned on by", generated from each module's `//! reads:` header (`option:comment_column`, `rules:[layout]`, `tag:#restyle`, `kinds:ctor_own_line_kinds`). An agent sees "comment-column-align | turned on by option comment_column | used by shell" and knows Java needs one `lang.yaml` line.
2. A codegen check: a `reads: option:X` header must name an option the module really reads (by id, which phase 1 makes checkable). The index then cannot go stale.
3. One line in adding-a-language.md and the agent brief: "A shared concept is turned on by the data in its index row; never write a switch or a second implementation."

Because there is no separate "disabled" state, there is nothing to discover beyond the index row. An option the language never sets is simply absent, the same as for a language that never existed.

## Phases

User decision 2026-11-19: approved, lint phase first (it blocks landings now). Order: 3, 1, 2, 4.

Each phase is one landing, with checks that do not depend on the bench alone:

1. **Options by id.** Codegen gives every option an id (language and shared lists); `ResolvedOptions` becomes a vector by id with borrowed list values; `get(&str)` is removed, so string lookups cannot come back. Policies are cached per file through the existing `FormatCtx::cached`. Check: every format parity run byte-identical (counts unchanged), second-format 0. Bench: no regression; format metrics should fall.
2. **Tag bitsets for every tag.** Check: same as 1.
3. **Lint: cached rule set, kind dispatch, embedded parse on demand (done).** Node dispatch by `kinds()`/`tags()` already existed; added `bodies()` for body dispatch, `reads_embedded()` and the `active_rules` query. Finding lists of 36 speed corpora identical before and after. Check: every coverage run reports the identical finding set before and after (a stronger check than the counts). Bench: lint metrics should fall, most for languages with many rules.
4. **Discoverability:** index column, `reads:` check, brief and checklist line (pattern 7: "a shared feature or rule costs languages that do not use it").

Phase 5 (decide after 1-3): a local allocation check per language in the gate, for earlier feedback than the CI bench. Against it: every legitimate shared change updates a baseline file, which is friction of its own. Measure first whether phases 1-3 stop the overruns; skip phase 5 if they do.

## Risks and what would make me stop

- **Merge conflicts.** Phase 1 touches about 60 call sites in core-format and the language formatters while 4 agents work there. Run it when the C/C++ landing is in, one phase per landing, and tell running agents before it lands.
- **Silent behaviour change in kind dispatch.** A rule that declares the wrong kinds stops reporting. The identical-findings check on every coverage corpus catches this; fixtures alone would not.
- **The gain may be small.** If phases 1-3 do not lower the bench numbers, the per-node lookups were not the cause. Stop after phase 1 and report. Today's overruns point at them, but this is not proven.
- **Not a full guarantee.** A shared feature can still add an unconditional walk or allocation (patterns 3-4 of the checklist). The plan removes the common causes, not every cause. The CI bench stays the final check.

## Cost

Phases 1-3: one agent step each. Phase 4: small, can ride with phase 3. Net code: roughly neutral (the string-keyed API and the ad hoc early exits go away; codegen of ids and the dispatch table come in).
