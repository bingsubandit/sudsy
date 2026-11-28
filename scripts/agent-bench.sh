#!/usr/bin/env bash
# The agent benchmark (docs/ai/design/agent-integration.md, step 1): headless Claude Code on pinned
# repositories under paired conditions. Costs money and needs a logged-in `claude`; not in CI.
#   scripts/agent-bench.sh [--smoke | --full] [--conditions native,sudsy,...] [--dry-run] [...]
#   python3 tools/agent-bench/stats.py tools/agent-bench/data/<batch>.jsonl
# Every option: scripts/agent-bench.sh --help. Task checks without a subject: tools/agent-bench/validate.py.
set -euo pipefail
exec python3 "$(cd "$(dirname "$0")/.." && pwd)/tools/agent-bench/bench.py" "$@"
