#!/usr/bin/env bash
# Lands a finished step with one headless Claude session (Haiku) on the API credit: scripts/land-agent.sh WORKTREE MSGFILE
# The session runs the existing scripts in order (fetch, rebase, codegen, gate --affected, finish --start/--wait, land.sh; the rules in
# scripts/land-agent.md) and resolves only the documented conflict cases. Needs SUDSY_API_KEY (no fallback to the subscription; the key
# goes into this one call's environment only). Cost cap: LAND_AGENT_BUDGET dollars (default 3). Full JSON of the run: WORKTREE/.cargo/tmp/land-agent.json.
# Prints one line: `PUSHED <hash>` (checked against origin/main) or `BLOCKED: <reason>`.
set -uo pipefail
wt=${1:-}; msg=${2:-}
[ -d "$wt" ] && [ -f "$msg" ] || { echo "BLOCKED: usage: scripts/land-agent.sh WORKTREE MSGFILE"; exit 1; }
[ -n "${SUDSY_API_KEY:-}" ] || { echo "BLOCKED: SUDSY_API_KEY is not set (this job runs on the API credit only, no fallback)"; exit 1; }
msg=$(realpath "$msg"); cd "$wt" || exit 1
mkdir -p .cargo/tmp
log=.cargo/tmp/land-agent.json
ANTHROPIC_API_KEY="$SUDSY_API_KEY" timeout 3600 claude -p --bare --model haiku --output-format json --no-session-persistence \
  --max-budget-usd "${LAND_AGENT_BUDGET:-3}" --max-turns 150 \
  --allowedTools "Bash(scripts/*)" "Bash(git *)" "Bash(cargo xtask *)" "Bash(CARGO_TARGET_DIR=* cargo xtask *)" "Bash(bash *)" Read Edit Grep Glob \
  --append-system-prompt "$(cat "$(dirname "$0")/land-agent.md")" \
  "Land this worktree. Message file: $msg" >"$log" 2>.cargo/tmp/land-agent.err
res=$(jq -r '.result // empty' "$log" 2>/dev/null)
[ -n "$res" ] || { echo "BLOCKED: no result from claude (see $wt/$log, $wt/.cargo/tmp/land-agent.err)"; exit 1; }
echo "land-agent: $(jq -r '"in=\(.usage.input_tokens) out=\(.usage.output_tokens) cache=\((.usage.cache_creation_input_tokens // 0) + (.usage.cache_read_input_tokens // 0)) usd=\(.total_cost_usd)"' "$log")" >&2
last=$(grep -v '^[[:space:]]*$' <<<"$res" | tail -1)
if [[ "$last" =~ ^PUSHED\ ([0-9a-f]{7,40})$ ]]; then
  git fetch -q origin main && git merge-base --is-ancestor "${BASH_REMATCH[1]}" origin/main && { echo "$last"; exit 0; }
  echo "BLOCKED: claimed $last, but that commit is not on origin/main"; exit 1
fi
echo "${last:+${last#BLOCKED: }}" | sed 's/^/BLOCKED: /'; exit 1
