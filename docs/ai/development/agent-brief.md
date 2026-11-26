# Agent brief

The standard brief of a build agent; a task prompt links it and adds only the task. [CLAUDE.md](../../../CLAUDE.md) holds the rules; this page is the routine.

## Working files
- First, write `.cargo/tmp/TASK.md` (gitignored, survives a restart): the whole prompt plus a progress section; update it as you go. A resumed agent reads it first.
- Write the slot to `.cargo/tmp/SLOT` (`/scratch/slots/sudsy/<n>`); the scripts read it before `CARGO_TARGET_DIR` (the inherited one names the lead's slot; [dev-loop.md](dev-loop.md#parallel-agents)). Plain `cargo` still needs the variable.
- Commit WIP often (`WIP: <what>`); `land.sh` squashes before the push.
- Multi-step commands (loops, heredocs, several git calls) go in `.cargo/tmp/*.sh`, written with Write, run with `bash`.
- PATH is set for agents: no PATH prefixes. No wait loops, `sleep`, `pgrep` or Monitor: `scripts/long.sh` and the finish rule in CLAUDE.md; never let a wait pass 5 minutes.
- Every turn rereads the whole context, so a turn with one `grep` is the most expensive way to learn one thing. Before each turn list what you need next and send all of it: several tool calls, or one Bash call with one read command per line (`grep` here, `sed -n` there, `git show` next to them). Read by line range, not whole files. Go alone only when the next command depends on this result. Merge these, each a turn saved every time: Write a script and run it; edit and run its check (`cargo check`, the fixture); grep with context (`grep -n -C8`) instead of a grep and then a `sed -n` of the hit; the `TASK.md` note and the next command; `git rebase` and `git status`.
- Write files with Write, not `cat > file <<EOF` (the worktree guard refuses many heredocs).
- Refused forms, each a wasted call: git inside `$(...)`, loops, heredocs or a long mixed command (plain git commands may be chained; the rest goes in a script file); `cargo` without `CARGO_TARGET_DIR=<slot>` in front, also in a command that only mentions cargo; `finish.sh --wait` piped.

## Rules in one line each
- HARD RULE: nothing is done in a language that can be generic ([CLAUDE.md](../../../CLAUDE.md)).
- Two-language rule: rows, row keys, tags and `format.rules` sections need two languages; rows hold facts, decisions stay in shared Rust ([CLAUDE.md](../../../CLAUDE.md)).
- Hard case: measure once (a prototype, an ablation), record numbers and the missing capability in [hard-cases.md](../design/hard-cases.md), move on. No unmeasured skips.
- No byte-compare of formatter output against a base build; an escaped regression becomes a test.
- A shared concept is turned on by the data in its row of [the kit index](../kit/index.md) (column `turned on by`: an option, a tag, a kind set, a rules section): set that data in the language, never write a switch or a second implementation.
- Parity goal: rank the uncovered findings per cause with counts before code, pick causes that sum to the goal, one finish per batch; a blocked landing reports its worth in points.
- Performance: check the change against [the checklist](../performance/checklist.md); on an allocation overrun run `perf-bench` there, not valgrind.

## Checks
- While working: `scripts/gate.sh -p <crate>` per changed crate; `cargo xtask codegen --check` after lang.yaml, grammar or registration changes; `cargo xtask dupes --check` for shared code.
- Parity step: start with the tool's section of [the queue](../parity/queue.md), classify before fixing (`name:N` in `files`, `fixes:` notes), keep unclassified under 15% of the differing files, take items from the top, commit the regenerated queue with each landing ([the queue](parity.md#the-queue)).
- Formatters: lossless, idempotent, meaning-preserving; parity ratchets only go down; `second-format` 0.
- Then `scripts/gate.sh --affected` once, then one finish: `scripts/finish.sh --start` (foreground, `timeout: 600000`; it dispatches and waits), then `--wait` (same timeout) while it prints `FINISH RUNNING`. No edits while it runs.
- Budgets: a unification step +5% on every metric while the plan is open; everything else as in CLAUDE.md. Above budget: keep the bench line, stop, report the numbers.

## Landing
After `FINISH OK`: write the commit message to a file (subject, 2-5 lines on why, the `Language-only:` / `Cost-exception:` lines if needed, the Co-Authored-By trailer), then `scripts/land.sh <file>`. It squashes (refuses after a fetch: rebase first), commits with `-F`, fetches and rebases, resolves `bench/history.jsonl` (both sides) and the generated `kit/index.md` and `single-use.txt` (`cargo xtask codegen`), stops for any other conflict (merge by hand, `git add`, `git rebase --continue`, run it again), refuses conflict markers, runs `codegen --check`, `dupes --check`, `cost.sh --languages --check origin/main HEAD` and, when a language crate grows, the HARD RULE review (a headless Haiku session with `scripts/review-hard-rule.md`; a MECHANISM finding refuses the landing: move it to a shared crate, or name a real fact as `Language-only: lang-<name>: <item>: <fact>`; output in `.cargo/tmp/review.txt`; a finding you hold to be wrong: stop and report it to the lead with the item and why; only the lead writes a `Lead-approved: lang-<name>: <item>: <reason>` line, which the review accepts), prints both stat views (`git show --stat HEAD`, `git diff origin/main --stat`: every deleted or shrunk file must be meant) and pushes `HEAD:main`, up to 3 rounds. Never `--ours`/`--theirs`, never a reset after a fetch.

## Mechanics
Mechanics (`.claude/agents/mechanic.md`, Haiku) take the work that a script or test checks; the lead starts them, and build agents for the cases under "Helpers".
- **Landing.** A build agent ends at a green `scripts/gate.sh -p` with everything committed on its branch, then hands over: branch and commit, slot, changed crates, the commit message in the branch's `.cargo/tmp/msg.txt` copied into the report, open questions. The mechanic resets its own worktree to that branch, rebases, runs codegen, gate and finish (the rules above) and `scripts/land.sh`. It stops and reports for: a conflict outside the files `land.sh` and the lead name, a failing gate (one rerun when it looks like the network), a bench regression above budget, or a line `land.sh` asks for. It keeps the bench line.
- **Measuring and setup.** It rebuilds corpora and tools, runs parity, coverage or stability and reports the command and the printed numbers.
- **Classifying.** It sorts listed pairs, rows or findings into given classes, one line each with the evidence. The lead checks a random sample; above 10% wrong the batch is discarded. Differing files of a format parity run go through `scripts/classify.py` instead ("API-credit jobs").
- **Data.** Rule texts, catalog rows, docs, fixtures from a given list; codegen and the gate must pass.

## API-credit jobs
Four routine Haiku jobs run as headless `claude -p --bare` calls on the `SUDSY_API_KEY` credit (`$100` a month), not on a session's subscription. `scripts/haiku.py` makes the call: the key goes into that call's environment only (`ANTHROPIC_API_KEY`), `--bare` never reads the subscription login, a missing key stops the script with a message (no fallback), and every call has a dollar cap (`--max-budget-usd`).
- **`scripts/classify.py <tool> run.log`**: the differing files of a format run (`PARITY_FILTER=<name> scripts/parity.sh format <tool> > run.log`) each get one gap name of the tool's `differences.yaml` or `unclassified` as JSON. Use it instead of reading diffs yourself, then `--results res.json --spot 5` (no call) and read those 5 diffs; above 1 of 10 wrong, discard. Cap per run: 20 files and 40,000 estimated input tokens (`--max-files`, `--max-tokens`, raise for a big batch; `--dry-run` prints the estimate), 5 files per call, $0.25 per call.
- **HARD RULE review** (inside `land.sh`): one call, fixed prompt `scripts/review-hard-rule.md`, structured output of `scripts/review-hard-rule.schema.json` (every item with file, function, class OK or MECHANISM, reason); any MECHANISM refuses the landing, the `Lead-approved` and `Language-only` lines work as before; cap $1.
- **`scripts/land-agent.sh WORKTREE MSGFILE`**: the landing of "Landing" as one headless Haiku session with tools (`scripts/land-agent.md`: existing scripts only, conflicts only `bench/history.jsonl` and the two generated files, anything else stops). Log `WORKTREE/.cargo/tmp/land-agent.json`; prints `PUSHED <hash>` (checked on origin/main) or `BLOCKED: <reason>`. Cap `LAND_AGENT_BUDGET` dollars (default 3). Same limits as a landing mechanic: a step with product code goes to the lead.
- **`scripts/token-check.py START [END]`**: counts of the session logs (cost units by model and kind, wakes: calls after more than 5 idle minutes) and a 5-line summary; only counts reach the prompt; $0.10 cap; `--dry-run` prints the counts without a key.
Cost of one run: `docs/ai/performance/numbers.md` ("API-credit jobs"); each call prints `in= out= cache= usd=` on stderr.

## Helpers
A build agent may start Haiku helpers with the Agent tool: one at a time, in the foreground, no other type than these two. The tool cannot enforce the list, so this rule does. They do not count as build agents.
- **`scout`**: a broad search (more than three files or an unknown place), or reading a long file, log or parity output. Ask one question and say what the answer is for; check the lines it names before editing them. A lookup of a known place is a grep of your own.
- **`mechanic`** in your worktree: corpus and tool setup, measuring and classifying files other than a format run's differences ("Mechanics", "API-credit jobs"), when it takes under 4 minutes or is a `scripts/long.sh` step. It never edits while you do.
- **Landing**: only of a step that changes docs, scripts, evidence or data and no product code or grammar, when the task prompt says so. Every other step is handed to the lead, who reviews it first. Start that mechanic with `isolation: worktree` and the hand-over of "Landing".

## Final report
One report per finished step, not per stage; a question only when the step cannot go on. Keep it under 40 lines: tables and lists in files, named in the report.

What changed; measured results (bench metrics, ratchets, `perf-bench` counts); `scripts/cost.sh` output; commits; open items and hard cases recorded.
