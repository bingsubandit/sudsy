# Agent workflows: what saves turns and tokens

Evidence for the agent features (MCP server, agent instructions, hooks, output formats): which
changes to the tools an agent calls reduce its turns, tokens and mistakes. Measured with mock tools
on a small fixture, before any of it is built in Sudsy. Harness, fixture and raw data:
[`tools/agent-ux/`](../../../tools/agent-ux/README.md). Measured 2026-11-27.

## Method

- **Subjects.** Real headless Claude Code runs: `claude -p "<TASK.md>" --model {haiku|sonnet}
  --output-format json` (Claude Code 2.1.332), one fresh git-initialised copy of the fixture per run,
  normal permission checks with a scoped `--allowedTools` list (file tools, the run's own `./`
  scripts, tests, read-only shell commands, `sed`, `mkdir`), hooks from `--settings` where a
  condition has them. Haiku 5.5 on every experiment, Sonnet 5.5 on the ones Haiku separated or left
  open.
- **Fixture.** A Python package `shop` (1,768 lines; `orders.py` alone 1,455), 13 plain-assert
  tests, deliberately unformatted modules and 41 pre-existing `ruff --select ALL` findings.
- **Conditions.** Each experiment keeps the task text identical and changes one thing: the mock tool
  (its output or behaviour), a hook, or one sentence of instructions. Condition A is the baseline.
  Mock tools are wrapper scripts over `black`, `ruff` and small Python checkers; `black` and `ruff`
  themselves are not on the subjects' allowlist, so a subject can only use the condition's tools.
- **Metrics.** From the run's JSON: `num_turns`, output tokens, input + cache tokens, cost, time.
  From `score.sh`: task tests, the experiment's own correctness fields (formatted, rename correct,
  lint clean, files changed outside the task), the mock tool calls (each tool logs its calls) and
  permission denials.
- **Repetitions.** 3 runs per condition and model (75 Haiku runs, 60 Sonnet runs, about $7 in all).
- **Separation rule.** A change against A is called **clear** when
  `|mean_X - mean_A| > sd_X + sd_A`. With n=3 this is crude: a change that is not clear is not shown
  to be absent, and changes of 10-20% in tokens or cost mostly need more runs to call (see Limits).
  `python3 tools/agent-ux/deltas.py data/*.jsonl` prints the tables below.
- **A first batch was invalid** (`data/haiku-v1-invalid.jsonl`, 2 runs per condition): 49 of 50
  runs hit permission denials (152 in all: pipes, `;`, heredocs, `python3 -c`, direct `black` and
  `ruff`), which added turns unevenly across conditions; for example the E8 gain it showed vanished
  once fixed. Every task now ends with the allowed commands and "one simple command per Bash call";
  the valid batches had 1 (Haiku) and 4 (Sonnet) denials.

## Experiments

Each row: change against condition A of the same experiment, mean of 3 runs; **clear** per the rule
above. Every run in the valid batches passed its task's correctness checks unless a row says
otherwise.

### E1: checks after edits (formatting staleness)

Question: does formatting the file after an edit (changing it under the agent) cost re-reads and
turns, and does feedback through a hook save the explicit check call?
Task: add `reorder_list(items, threshold)` to `shop/inventory.py` (lint clean, unformatted for
black); "code is formatted with black, new code has no ruff findings".

| Cond | What the agent gets | Haiku turns / out / in / cost | Sonnet turns / out / in / cost | Formatted at end (H, S) |
|---|---|---|---|---|
| A | `./check FILE`: formats in place, prints lint findings or `ok` | 8.0 / 2,237 / 169k / $0.0055 | 8.3 / 1,583 / 218k / $0.092 | 2/3, 0/3 |
| B | A plus the diff black applied | +25% / +31% / +29% / +22% | −16% clear / −17% / −20% / −11% clear | 0/3, 0/3 |
| C | `./check`: lint only, never formats | +25% / −12% / +33% / +3% | −28% / −42% / −26% / −26%, all clear | 0/3, 0/3 |
| D | no check; PostToolUse hook formats + lints the edited file, silent when clean | −8% / −27% / +24% / −3% | −36% / −39% / −20% / −25% (all but in clear) | 3/3, 3/3 |
| E | no check; hook lints only, a Stop hook formats at the end | −17% / −41% / −10% / −20% | **−48% / −59% / −39% / −34%, all clear** | 3/3, 3/3 |

Findings: hooks win on both models, most with lint per edit and one format at the end (E). The
staleness cost of formatting in place is visible on Sonnet (A is the worst condition) but not
clearly on Haiku. The larger effect is correctness: when formatting is a command the agent runs,
the file ended unformatted in 10 of the 12 runs of A and B (Haiku 4 of 6, Sonnet 6 of 6), because the
agent ran the check and then edited again. With hooks the file was always formatted.

### E2: only the findings this change introduced

Question: does printing pre-existing findings cost tokens or tempt off-task fixes?
Task: add `active_users(users)` to `shop/users.py`; "must not introduce new lint findings; findings
that already existed are not your job". A: `./lint` prints all 41 repo findings. B: only findings
not in a baseline (matched by file, rule and line text), nothing at all when clean.

| Cond | Haiku turns / out / in / cost | Sonnet turns / out / in / cost |
|---|---|---|
| A | 8.0 / 1,775 / 214k / $0.0057 | 6.7 / 687 / 152k / $0.076 |
| B | −17% / **−42% clear** / −29% / **−28% clear** | +0% / +7% / +6% / **−15% clear** |

No subject fixed a pre-existing finding in either condition (the task forbade it, so scope creep is
not tested here). Clear token and cost savings on Haiku, cost only on Sonnet.

### E3: message style and fix by id

Question: do messages that say what to do (with the source line and the exact replacement) save
reads and turns, and does applying a fix by id save output tokens?
Task: make `./lint shop/pricing.py` silent (6 seeded violations of 3 project rules: `print` instead
of `log.info`, float literal instead of `Decimal`, http call without `timeout=`), tests still pass.

| Cond | Lint output | Haiku turns / out / in / cost | Sonnet turns / out / in / cost |
|---|---|---|---|
| A | `path:line:col R1 print call` | 19.0 / 4,858 / 453k / $0.0111 | 14.3 / 2,178 / 250k / $0.102 |
| B | rule text saying what to use + offending line + `fix:` with the replacement | **−18% / −25% / −25% / −24%, all clear** | **−19% clear / −13% clear / −7% / −9% clear** |
| C | A plus an id per finding; `./fix ID` applies the fix | −4% / **−37% clear** / +34% / −5% | −5% / **−49% clear** / +14% clear / −11% clear |

Actionable messages save turns and tokens on both models. Fix by id roughly halves output tokens
(the model stops writing replacements) but saves no turns and costs input tokens (one call per id).

### E4: structural search instead of grep

Question: does a search that returns exactly the real call sites beat grep on a noisy name?
Task: rename `Cache.get` to `Cache.fetch` (definition + 8 call sites in 4 files) without touching
50 decoys (dict `.get`, `client.get`, `get_user`, comments, strings). A: grep. B: `./sgrep
Cache.get` prints the 9 real sites.

| Cond | Haiku turns / out / in / cost | Sonnet turns / out / in / cost |
|---|---|---|
| A | 22.0 / 5,209 / 334k / $0.0102 | 16.3 / 2,897 / 218k / $0.120 |
| B | −2% / −11% / −5% / −10% | −10% / −35% / −12% / −24% |

All 12 runs renamed correctly with no decoy touched. Sonnet's B runs took 9, 10 and 25 turns, so
nothing is clear. The fixture is too easy: the decoys are distinguishable by receiver name with
grep. Open: a fixture where grep cannot tell (same receiver names, inheritance, re-exports).

### E5: outline of a large file (Haiku only)

Question: does a signatures-only view save reading a 1,455-line file?
Task: add `OrderBook.total_with_tax(region)` using three helpers defined far apart, rounding once at
the end. B: `./outline FILE` (line, signature, one-line docstring).
Haiku: turns +8%, out −20%, in −16%, cost +7%; nothing clear, all correct. Open: a task where the
agent cannot just grep the helper names it was given.

### E6: project rules given up front (Haiku only)

Question: do rules written in the instructions prevent violations, saving the fix round?
Task: add `refund(order_id, amount)` to `shop/pricing.py`, make `./lint` pass. B adds `RULES.md`
and "the project rules are in RULES.md". Haiku: turns +0%, out +3%, in −11%, cost −17% (A's cost
varies ±30%); nothing clear. The terse lint feedback was enough to converge in both.

### E7: one check command instead of three

Question: do agents chain separate tools (`a && b && c`), or does one command save turns?
Task: make the repo pass formatting, lint and typecheck (3 seeded type errors), tests pass. A:
`./fmt-check`, `./lint`, `./typecheck`. B: one `./check` printing the three groups.

| Cond | Haiku turns / out / in / cost | Sonnet turns / out / in / cost |
|---|---|---|
| A | 29.3 / 16,412 / 563k / $0.0202 | 15.7 / 4,099 / 245k / $0.131 |
| B | −33% / **−20% clear** / −21% / **−19% clear** | **−36% clear / −13% clear** / −7% / **−8% clear** |

The largest turn saving measured, on both models (Haiku's A varies 22-33 turns, so its turns are
not clear by the rule). Subjects called the three tools as separate steps, not chained.

### E8: collapsed output

Question: does collapsing many findings (one line per rule with a count and 3 locations, a cap of
40 lines, `./lint --rule R` and `./explain R` on demand) save tokens or turns?
Task: fix the R2 and R3 findings in `shop/` and leave R1 (107 R1, 10 R2, 10 R3 findings). A: every
finding with a 4-line explanation (625 lines).

| Cond | Haiku turns / out / in / cost | Sonnet turns / out / in / cost |
|---|---|---|
| A | 29.3 / 7,739 / 558k / $0.0157 | 20.3 / 2,824 / 505k / $0.167 |
| B | +11% / −8% / **−21% clear / −17% clear** | +10% / +9% / **−38% clear / −23% clear** |

Collapsing saves input tokens and cost clearly (−38% input on Sonnet) but not turns: the drill-down
call costs one. No subject touched R1 findings in either condition.

### E9: the agent writes an enforced rule (Haiku only, one condition)

Question: given "never call `datetime.now()` without a timezone in shop/; make that an enforced
rule", does the agent write a working pattern rule (`rules/*.yaml`: id, message, regex `pattern`,
optional `not-pattern`, paths)?
Scored on `shop/clock.py`: 4 bad uses to flag, 4 good ones not to (`now(tz)`, `now(UTC)`, a comment,
a string). Haiku: 3/3 runs exact, 13.3 turns, $0.013.

### E10: a check that reports names that do not exist

Question: does reporting an unknown attribute stop the agent building on an API that does not exist?
Task: implement `user_names(ids, cache)` in `shop/report.py` "using `Cache.get_many(keys)`", which
does not exist (Cache has get, set, items, clear, get_stats). A: tests only. B: `./check` also
prints `shop/report.py:10: Cache has no attribute get_many (has: clear, get, get_stats, items, set)`.

| Cond | Haiku turns / out / in / cost | Sonnet turns / out / in / cost | Added `get_many` to `Cache` (H, S) |
|---|---|---|---|
| A | 10.3 / 2,017 / 267k / $0.0062 | 8.7 / 1,379 / 204k / $0.081 | 3/3, 3/3 |
| B | +6% / +2% / −6% / −3% | −8% / −9% / −13% / −5% | 2/3, **0/3** |

No cost or turn effect, but a behaviour effect on Sonnet: with the check it used the existing API
in all 3 runs instead of adding a method to a shared class it was not asked to change. Adding it is
a defensible fix, since the task named it, so the fixture cannot call either a mistake. Open: a task
where the invented name is not in the instructions.

### E11: "never spend effort on formatting"

Question: does one instruction sentence ("Formatting is automatic; never spend effort on
whitespace, quotes or line length.") cut output tokens? Both conditions use E1/D (format hook).

| Cond | Haiku turns / out / in / cost | Sonnet turns / out / in / cost |
|---|---|---|
| A (E1/D without the sentence) | 7.3 / 1,737 / 211k / $0.0054 | 4.7 / 812 / 147k / $0.063 |
| B | −14% / **−37% clear / −21% clear / −17% clear** | +0% / −11% / −9% / +0% |

Clear savings on Haiku; Sonnet already writes little and shows none.

### E12: a whitespace-free "minimized" view of source

Question: how many tokens would agents save reading and writing code with no indentation?
No subject: token counts of 5 fixture files in a fixed one-line prompt (Haiku tokenizer, minus a
baseline without a file), black-formatted vs indentation and blank lines stripped vs one tab per
level: stripped saves 3.9-7.7% (orders.py 18,472 → 17,756 tokens), tabs +1.3% to −0.3%. Runs of
spaces are already cheap tokens. Not pursued: the saving is small, it breaks indentation-sensitive
languages, and Claude Code's Edit tool matches exact file text.

## Conclusions

Clear on both models:
1. **Feedback through hooks** (E1 D, E): fewer turns and tokens on Sonnet (−48% turns, −34% cost
   for lint-per-edit + format-at-stop) and the file always ends formatted, which formatting as a
   command (E1 A, B) failed to do in 10 of 12 runs.
2. **One command for every check** (E7): −36% turns on Sonnet, −8% to −19% cost.
3. **Messages that say what to do**, with the line and the exact fix (E3 B): about −19% turns.

Clear on tokens or cost, not turns: only new findings (E2: Haiku −28% cost, Sonnet −15%),
collapsed output (E8: −17% to −23% cost), fix by id (E3 C: −37% to −49% output tokens), the
"never format" sentence (E11, Haiku only: −17% cost).

Behaviour, not cost: an unknown-name check kept Sonnet from adding an unrequested method (E10).
Feasible: agents write correct pattern rules from a one-sentence policy (E9).

Not shown (fixtures too easy, or no effect at n=3): structural search (E4), outline (E5), rules up
front (E6). Rejected: the minimized format (E12).

## Limits and next steps

- n=3 separates effects of roughly 20% and more; the 10-20% token and cost changes marked not clear
  above (E4 Sonnet −24% cost, E5 −20% output, E6 −17% cost) need about 8-10 runs per condition.
- Small tasks (4-33 turns) on a 1,768-line fixture; effects that grow with repo size (search,
  outline, noise from old findings) are understated.
- Mock tools stand in for features Sudsy does not have yet. The next round runs the same tasks
  against the real implementations (a single `sudsy check`, the hook setup, changed-only output,
  actionable messages), with more runs per condition and harder fixtures for E4, E5 and E10.
