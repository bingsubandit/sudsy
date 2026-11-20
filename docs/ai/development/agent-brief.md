# Agent brief

The standard brief of a build agent; a task prompt links it and adds only the task. [CLAUDE.md](../../../CLAUDE.md) holds the rules; this page is the routine.

## Working files
- First, write `.cargo/tmp/TASK.md` (gitignored, survives a restart): the whole prompt plus a progress section; update it as you go. A resumed agent reads it first.
- Write the slot to `.cargo/tmp/SLOT` (`/scratch/slots/sudsy/<n>`); the scripts read it before `CARGO_TARGET_DIR` (the inherited one names the lead's slot; [dev-loop.md](dev-loop.md#parallel-agents)). Plain `cargo` still needs the variable.
- Commit WIP often (`WIP: <what>`); `land.sh` squashes before the push.
- Multi-step commands (loops, heredocs, several git calls) go in `.cargo/tmp/*.sh`, written with Write, run with `bash`. One plain command per call otherwise.
- PATH is set for agents: no PATH prefixes. No wait loops, `sleep` or Monitor: see the finish and background rules in CLAUDE.md.

## Rules in one line each
- HARD RULE: nothing is done in a language that can be generic ([CLAUDE.md](../../../CLAUDE.md)).
- Two-language rule: rows, row keys, tags and `format.rules` sections need two languages; rows hold facts, decisions stay in shared Rust ([CLAUDE.md](../../../CLAUDE.md)).
- Hard case: measure once (a prototype, an ablation), record numbers and the missing capability in [hard-cases.md](../design/hard-cases.md), move on. No unmeasured skips.
- No byte-compare of formatter output against a base build; an escaped regression becomes a test.
- Performance: check the change against [the checklist](../performance/checklist.md); on an allocation overrun run `perf-bench` there, not valgrind.

## Checks
- While working: `scripts/gate.sh -p <crate>` per changed crate; `cargo xtask codegen --check` after lang.yaml, grammar or registration changes; `cargo xtask dupes --check` for shared code.
- Parity step: start with the tool's section of [the queue](../parity/queue.md), classify before fixing (`name:N` in `files`, `fixes:` notes), keep unclassified under 15% of the differing files, take items from the top, commit the regenerated queue with each landing ([the queue](parity.md#the-queue)).
- Formatters: lossless, idempotent, meaning-preserving; parity ratchets only go down; `second-format` 0.
- Then `scripts/gate.sh --affected` once, then one finish: `scripts/finish.sh --start`, then `--wait` (foreground, `timeout: 600000`) until it is not `FINISH RUNNING`. No edits while it runs.
- Budgets: a unification step +5% on every metric while the plan is open; everything else as in CLAUDE.md. Above budget: keep the bench line, stop, report the numbers.

## Landing
After `FINISH OK`: write the commit message to a file (subject, 2-5 lines on why, the `Language-only:` / `Cost-exception:` lines if needed, the Co-Authored-By trailer), then `scripts/land.sh <file>`. It squashes (refuses after a fetch: rebase first), commits with `-F`, fetches and rebases, resolves `bench/history.jsonl` (both sides) and the generated `kit/index.md` and `single-use.txt` (`cargo xtask codegen`), stops for any other conflict (merge by hand, `git add`, `git rebase --continue`, run it again), refuses conflict markers, runs `codegen --check`, `dupes --check`, `cost.sh --languages --check origin/main HEAD` and, when a language crate grows, the HARD RULE review (a headless Haiku session with `scripts/review-hard-rule.md`; a MECHANISM finding refuses the landing: move it to a shared crate, or name a real fact as `Language-only: lang-<name>: <item>: <fact>`; output in `.cargo/tmp/review.txt`; a finding you hold to be wrong: stop and report it to the lead with the item and why; only the lead writes a `Lead-approved: lang-<name>: <item>: <reason>` line, which the review accepts), prints both stat views (`git show --stat HEAD`, `git diff origin/main --stat`: every deleted or shrunk file must be meant) and pushes `HEAD:main`, up to 3 rounds. Never `--ours`/`--theirs`, never a reset after a fetch.

## Final report
What changed; measured results (bench metrics, ratchets, `perf-bench` counts); `scripts/cost.sh` output; commits; open items and hard cases recorded.
