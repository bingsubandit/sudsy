"""merge.py RUNDIR result.json -> score JSON + subject metrics (one line)."""
import json, subprocess, sys, pathlib

run, res = sys.argv[1], sys.argv[2]
here = pathlib.Path(__file__).resolve().parent
s = json.loads(subprocess.run(["python3", "-I", str(here / "score.py"), run], capture_output=True, text=True).stdout)
try:
    r = json.loads(pathlib.Path(res).read_text())
except Exception as e:  # noqa
    r = {"parse_error": str(e)}
u = r.get("usage") or {}
s["num_turns"] = r.get("num_turns")
s["total_cost_usd"] = r.get("total_cost_usd")
s["duration_ms"] = r.get("duration_ms")
s["output_tokens"] = u.get("output_tokens")
s["input_plus_cache_tokens"] = sum(u.get(k) or 0 for k in ("input_tokens", "cache_creation_input_tokens", "cache_read_input_tokens"))
s["is_error"] = r.get("is_error", True)
s["permission_denials"] = r.get("permission_denials", [])
if "parse_error" in r:
    s["result_parse_error"] = r["parse_error"]
print(json.dumps(s))
