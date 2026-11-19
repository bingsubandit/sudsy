#!/usr/bin/env python3
"""The job matrix of .github/workflows/nightly.yml as JSON: one entry per parity tool
(scripts/parity_tools.json) and per crate with ignored conformance tests (not measurements)."""
import json
import re
import sys
from pathlib import Path

root = Path(__file__).resolve().parent.parent
skip = re.compile(r"measurement|prints timings|flaky")
include = []
modes = json.load(open(root / "scripts/parity_tools.json"))
for mode in ("format", "lint"):  # the other sections (speed-only tools, corpora) have no parity run
    for tool in modes[mode]:
        cmd = f"scripts/parity.sh {'format ' if mode == 'format' else ''}{tool}"
        include.append({"name": f"parity-{mode}-{tool}", "cmd": cmd})
for crate in sorted((root / "crates").glob("lang-*")):
    files = [*crate.glob("tests/*.rs"), *crate.glob("src/**/*.rs")]
    if any("#[ignore" in l and not skip.search(l) for f in files for l in f.read_text().splitlines()):
        include.append({"name": f"conformance-{crate.name}", "cmd": f"scripts/conformance.sh {crate.name}"})
json.dump({"include": include}, sys.stdout)
print()
