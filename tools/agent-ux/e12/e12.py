"""E12: input-token cost of file layouts (haiku, no tools). Writes e12/results.json."""
import json, pathlib, re, subprocess, tempfile, shutil

ROOT = pathlib.Path(__file__).resolve().parent.parent
tmp = pathlib.Path(tempfile.mkdtemp(prefix="e12-"))
FILES = ["orders", "pricing", "inventory", "users", "cache"]
INSTR = "Reply with the single word ok and nothing else."


def black(src):
    p = tmp / "x.py"; p.write_text(src)
    subprocess.run(["black", "-q", str(p)], check=False)
    return p.read_text()


def strip(src):
    return "\n".join(l.lstrip() for l in src.splitlines() if l.strip()) + "\n"


def tabs(src):
    return "\n".join(re.sub(r"^((?:    )+)", lambda m: "\t" * (len(m.group(1)) // 4), l) for l in src.splitlines()) + "\n"


def tokens(prompt):
    r = subprocess.run(["claude", "-p", prompt, "--model", "haiku", "--output-format", "json", "--tools", ""],
                       cwd=tmp, capture_output=True, text=True, stdin=subprocess.DEVNULL)
    d = json.loads(r.stdout)
    u = d["usage"]
    return u["input_tokens"] + u["cache_creation_input_tokens"] + u["cache_read_input_tokens"], d["total_cost_usd"]


base, c0 = tokens(INSTR)
out = {"baseline_tokens": base, "files": {}}
total = c0
for f in FILES:
    src = black((ROOT / "base" / "shop" / f"{f}.py").read_text())
    row = {"lines": len(src.splitlines()), "chars": len(src)}
    for name, fn in (("a_black", lambda s: s), ("b_stripped", strip), ("c_tabs", tabs)):
        text = fn(src)
        t, c = tokens(f"{INSTR}\n\n<file>\n{text}</file>")
        total += c
        row[name] = t - base
        row[name + "_chars"] = len(text)
    out["files"][f] = row
out["total_cost_usd"] = total
(ROOT / "e12" / "results.json").write_text(json.dumps(out, indent=1))
print(json.dumps(out, indent=1))
