# Languages with no reference tool

How a language plan (`docs/ai/design/<lang>.md`) decides what to format and lint when no tool exists to compare with. It applies per leg: a language can have a reference linter and no formatter (Dockerfile). The absence of a tool never holds a language back. What replaces the tool is written evidence, fixed before anything is built, so the result cannot be fitted to what is easy.

## 1. Show that there is none
The plan lists where it looked (package registries, editor extensions, language server, mega-linter, pre-commit hooks, conform.nvim, GitHub topic search) and what was found. Anything usable is a source below, even if it is partial, unmaintained or an editor feature:

| Source | Gives |
|---|---|
| The language's own parser, compiler or interpreter | oracle for meaning (before and after give the same result) and its warnings as lint rules |
| Specification, official manual, official style guide | layout rules, documented pitfalls |
| Partial tool, language server, editor formatter | a comparison for the part it covers; it is the primary tool for that part |
| Real code | what people write (the consensus below) |
| A close language we support | rules and options whose reason carries over, never its style by default |

## 2. Corpus
At least 300 files from at least 20 unrelated projects, pinned by commit, chosen by project before any result is seen (fewer only if the language has fewer; the plan says so). Half of the projects (chosen at random, recorded) are for deciding, half are held out for checking.

## 3. Format
1. **Decision table.** One row per layout choice the formatter makes (indent, blanks around operators, case of keywords, alignment, order, wrapping, blank lines). Each has its evidence, first that applies: the specification or official guide; a partial tool; the corpus; a close language.
2. **Corpus rule.** Count the variants per project (one vote per project, so a large one does not decide) on the deciding half. One variant in at least 80% of the projects: it is the default. Between 60 and 80%: it is the default and a named option covers the other. Below 60%: the source is kept as written, or an option with "as written" as the default.
3. **Default to leaving it.** A choice with no evidence is not made.
4. **Number.** Two, both needed: `stable` (`scripts/parity.sh stability`, target 100% of the files that parse), and **agreement**: on the held-out half, the share of occurrences of each decided construct that already have the chosen form, at least 80% overall. Files the formatter changes are sampled (30) and read for harm.
5. **Meaning oracle.** Where the language has a runnable parser or interpreter: it gives the same syntax tree, tokens or output before and after, on the whole corpus. No oracle: the plan says so, and the formatter changes only blanks, line ends and case where the specification says they carry no meaning.

## 4. Lint
1. **Checklist first.** A fixed list of problems, from the specification (undefined, deprecated, error), documented pitfalls of the manual and well-known guides, warnings of the runtime, partial tools, and fix commits of the corpus projects (a search of their history). Every entry has a source link and a smallest bad and good example; entries that need to run the code, the network or the project's environment are named and left out. The lead reviews the list before any rule is built.
2. **Number.** `checklist`: the share of the entries that a rule reports on the bad example and not on the good one, target 85%. The entries are the denominator; a rule built without an entry does not count until its entry is added with a source.
3. **Precision.** Every rule runs over the corpus. Of its findings, up to 50 at random are judged true or false by a reader that did not write the rule; below 90% true it is fixed or off by default. The judged sample is committed with the numbers.
4. **Yield.** A rule with no finding on the corpus is kept only with a source from the specification or the runtime.

## 5. What the plan contains
Sources found, corpus and split, decision table, checklist, the oracle or its absence, what is left out and why, and the checkpoint (lead review of table and list before building). Choices of taste that the evidence does not decide are listed for the user, with a recommendation.

## 6. Ready numbers
A new language's leg with no reference tool is `Ready` in the README table at: format `stable` 100% and `agreement` 80%; lint `checklist` 85% and `precision` 90% (the numbers of sections 3 and 4). The row of `xtask/full/support.yaml` names `evidence: <folder>`, a folder of `tests/parity` whose `differences.yaml` holds `agreement`, and `checklist` with `precision` (both or none), in percent; `cargo xtask codegen --check` fails a non-exempt format leg with no reference tool that lacks `agreement`. The five legs that predate this page are `exempt: true` by name and keep the `stable` bar of 85%: INI, EditorConfig and .gitignore format (one row), Dockerfile and GitHub Actions format. They are scored as `retrofit` rows of the priority table (`xtask/full/languages.yaml`: parser paid, the cost of what this page still asks of them), compete with the new-language rows and never rank ahead of a primary leg under the bar.

## Tooling
`scripts/evidence.py <language> ...` is the shared harness of sections 2 to 5: file list, seeded project split, one vote per project, agreement, comparison with a partial tool. A language adds `scripts/evidence_<language>.py` with its facts only (corpus folders, seed, row variants, causes of difference); examples: `evidence_dockerfile.py`, `evidence_makefile.py`.

## 7. When a tool appears
A maintained reference tool that turns up later becomes the primary tool of the leg. Its result is compared as for any other language, and the tables here keep the options.
