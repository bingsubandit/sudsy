"""One headless `claude -p` call on the API credit (SUDSY_API_KEY), shared by classify.py, token-check.py and land.sh.

The key goes only into the environment of the one call (`ANTHROPIC_API_KEY`), never into this process's environment, a file or a
prompt. `--bare` makes Claude Code read no OAuth login, so the subscription is never used. A missing key stops with a message; there
is no fallback. Every call has a dollar cap (`--max-budget-usd`).

  haiku.py [--schema FILE] [--system FILE] [--tools "Read Grep Glob"] [--budget USD] [--timeout S] PROMPT    (stdin is passed on)

prints the structured output (or the text) on stdout and `haiku: in=.. out=.. cache=.. usd=..` on stderr.
"""
import argparse
import json
import os
import subprocess
import sys

MODEL = "haiku"
KEY = "SUDSY_API_KEY"


class HaikuError(Exception):
    pass


def require_key():
    if not os.environ.get(KEY):
        raise HaikuError(f"{KEY} is not set: this job runs on the API credit only and never falls back to the subscription")


def usage_line(res):
    u = res.get("usage") or {}
    cache = u.get("cache_creation_input_tokens", 0) + u.get("cache_read_input_tokens", 0)
    return f"in={u.get('input_tokens', 0)} out={u.get('output_tokens', 0)} cache={cache} usd={res.get('total_cost_usd', 0):.4f}"


def tokens(res):
    u = res.get("usage") or {}
    return sum(u.get(k, 0) for k in ("input_tokens", "output_tokens", "cache_creation_input_tokens", "cache_read_input_tokens"))


def build(schema=None, system=None, tools="", budget=0.5, extra=()):
    cmd = ["claude", "-p", "--bare", "--model", MODEL, "--output-format", "json", "--no-session-persistence",
           "--max-budget-usd", str(budget)]
    if tools:
        cmd += ["--allowedTools", *tools.split()]
    else:
        cmd += ["--tools", ""]
    if schema is not None:
        cmd += ["--json-schema", schema if isinstance(schema, str) else json.dumps(schema)]
    if system:
        cmd += ["--append-system-prompt", system]
    return cmd + list(extra)


def call(prompt, *, stdin=None, cwd=None, timeout=600, **kw):
    """Runs the call; returns the parsed `--output-format json` object plus `data` (structured output, else the text parsed as JSON,
    else None). Raises HaikuError on a missing key, a failed call or an error result."""
    require_key()
    env = dict(os.environ, ANTHROPIC_API_KEY=os.environ[KEY])
    try:
        p = subprocess.run(build(**kw) + [prompt], input=stdin, capture_output=True, text=True, env=env, cwd=cwd, timeout=timeout)
    except subprocess.TimeoutExpired:
        raise HaikuError(f"claude -p timed out after {timeout}s")
    try:
        res = json.loads(p.stdout)
    except ValueError:
        raise HaikuError(f"claude -p exit {p.returncode}, no JSON: {(p.stdout + p.stderr)[:300]}")
    if res.get("is_error") or p.returncode not in (0,):
        raise HaikuError(f"claude -p failed: {str(res.get('result'))[:300]}")
    data = res.get("structured_output")
    if data is None:
        text = (res.get("result") or "").strip()
        if text.startswith("```"):
            text = text.split("\n", 1)[-1].rsplit("```", 1)[0]
        try:
            data = json.loads(text)
        except ValueError:
            data = None
    res["data"] = data
    return res


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("prompt")
    ap.add_argument("--schema")
    ap.add_argument("--system")
    ap.add_argument("--tools", default="")
    ap.add_argument("--budget", type=float, default=0.5)
    ap.add_argument("--timeout", type=int, default=600)
    a = ap.parse_args()
    schema = open(a.schema).read() if a.schema else None
    system = open(a.system).read() if a.system else None
    stdin = None if sys.stdin.isatty() else sys.stdin.read()
    try:
        res = call(a.prompt, stdin=stdin, schema=schema, system=system, tools=a.tools, budget=a.budget, timeout=a.timeout)
    except HaikuError as e:
        print(f"haiku: {e}", file=sys.stderr)
        return 2
    print(f"haiku: {usage_line(res)}", file=sys.stderr)
    print(json.dumps(res["data"]) if res["data"] is not None else res.get("result", ""))
    return 0


if __name__ == "__main__":
    sys.exit(main())
