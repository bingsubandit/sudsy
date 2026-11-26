"""Tests of the API-credit jobs (haiku.py, classify.py, token-check.py, land-agent.sh) with a fake `claude` on PATH; part of
`scripts/parity.sh --test`."""
import contextlib
import io
import json
import os
import stat
import subprocess
import tempfile
import unittest
from pathlib import Path

import classify
import haiku
import importlib

token_check = importlib.import_module("token-check")

ROOT = Path(__file__).resolve().parent.parent
FAKE = """#!/usr/bin/env python3
import json, os, sys
open(os.environ["FAKE_LOG"], "a").write(json.dumps({"key": os.environ.get("ANTHROPIC_API_KEY"), "argv": sys.argv[1:]}) + "\\n")
sys.stdin.read() if not sys.stdin.isatty() else None
print(os.environ["FAKE_OUT"])
"""
LOG = """a.txt: DIFFERENT
--- a.txt
+++ sudsy
-x
+y
b.txt: known difference (g1)
c.txt: DIFFERENT
-1
+2
"""


@contextlib.contextmanager
def fake(out, key="sk-test"):
    with tempfile.TemporaryDirectory() as d:
        exe = Path(d, "claude")
        exe.write_text(FAKE)
        exe.chmod(exe.stat().st_mode | stat.S_IXUSR)
        env = {"PATH": d + os.pathsep + os.environ["PATH"], "FAKE_LOG": str(Path(d, "log")), "FAKE_OUT": json.dumps(out)}
        if key:
            env["SUDSY_API_KEY"] = key
        saved = dict(os.environ)
        os.environ.pop("SUDSY_API_KEY", None)
        os.environ.pop("ANTHROPIC_API_KEY", None)
        os.environ.update(env)
        try:
            yield Path(d, "log")
        finally:
            os.environ.clear()
            os.environ.update(saved)


def result(data):
    return {"is_error": False, "result": "", "structured_output": data, "total_cost_usd": 0.01,
            "usage": {"input_tokens": 10, "output_tokens": 5, "cache_read_input_tokens": 100}}


class HaikuTests(unittest.TestCase):
    def test_missing_key_stops_without_a_call(self):
        with fake(result({}), key=None) as log:
            with self.assertRaises(haiku.HaikuError):
                haiku.call("p")
            self.assertFalse(log.exists())

    def test_key_only_in_the_call_environment(self):
        with fake(result({"ok": 1})) as log:
            res = haiku.call("p", schema={"type": "object"}, tools="Read Grep")
            self.assertEqual(res["data"], {"ok": 1})
            self.assertNotIn("ANTHROPIC_API_KEY", os.environ)
            call = json.loads(log.read_text())
            self.assertEqual(call["key"], "sk-test")
            self.assertIn("--bare", call["argv"])
            self.assertIn("--max-budget-usd", call["argv"])
            self.assertEqual(haiku.tokens(res), 115)

    def test_error_result_raises(self):
        with fake({"is_error": True, "result": "boom"}):
            with self.assertRaises(haiku.HaikuError):
                haiku.call("p")


class ClassifyTests(unittest.TestCase):
    def folder(self, d):
        Path(d, "tests/parity/t").mkdir(parents=True)
        Path(d, "tests/parity/t/differences.yaml").write_text("differences:\n  g1: gap  text one\n  g2: choice  text two\n")

    def test_sections_take_different_files_only(self):
        s = classify.sections(LOG)
        self.assertEqual(sorted(s), ["a.txt", "c.txt"])
        self.assertIn("+y", s["a.txt"])

    def test_caps_and_estimate(self):
        files = {f"f{i}": "x" * 300 for i in range(10)}
        batches, skipped, _ = classify.plan(files, 50, 4, 10**6, 3)
        self.assertEqual([len(b) for b in batches], [3, 1])
        self.assertEqual(skipped, 6)
        _, skipped, _ = classify.plan(files, 50, 99, 250, 3)  # 300 chars = 101 tokens: two fit
        self.assertEqual(skipped, 8)

    def test_classify_validates_names_and_dry_run_needs_no_key(self):
        with tempfile.TemporaryDirectory() as d, fake(result({"classifications": [
                {"file": "a.txt", "gap": "g1"}, {"file": "c.txt", "gap": "invented"}]})) as log:
            self.folder(d)
            Path(d, "run.log").write_text(LOG)
            out = Path(d, "res.json")
            with contextlib.redirect_stdout(io.StringIO()):
                classify.main(["t", f"{d}/run.log", "--root", d, "--out", str(out)])
            self.assertEqual(json.loads(out.read_text()), {"a.txt": "g1", "c.txt": "unclassified"})
            self.assertEqual(len(log.read_text().splitlines()), 1)  # one batch
            buf = io.StringIO()
            with contextlib.redirect_stdout(buf):
                classify.main(["t", f"{d}/run.log", "--results", str(out), "--spot", "5"])
            self.assertIn("=== a.txt: g1", buf.getvalue())
        with tempfile.TemporaryDirectory() as d, fake(result({}), key=None) as log:
            self.folder(d)
            Path(d, "run.log").write_text(LOG)
            classify.main(["t", f"{d}/run.log", "--root", d, "--dry-run"])
            with self.assertRaises(SystemExit):
                classify.main(["t", f"{d}/run.log", "--root", d])
            self.assertFalse(log.exists())


class TokenCheckTests(unittest.TestCase):
    def test_counts_hold_no_names_and_find_wakes(self):
        with tempfile.TemporaryDirectory() as d:
            u = {"input_tokens": 1000, "output_tokens": 100, "cache_creation_input_tokens": 0, "cache_read_input_tokens": 0}
            rows = [{"type": "assistant", "timestamp": f"2026-01-01T00:{m:02d}:00Z", "message": {
                "id": f"m{m}", "model": "claude-haiku-x", "usage": u, "content": [{"type": "tool_use", "name": "Bash", "input": {"command": "git status"}}]}}
                for m in (0, 1, 30)]
            Path(d, "secret-session.jsonl").write_text("\n".join(json.dumps(r) for r in rows))
            buf = io.StringIO()
            with contextlib.redirect_stdout(buf):
                token_check.main(["2025-01-01T00:00:00Z", "--root", d, "--dry-run"])
            c = json.loads(buf.getvalue())
            self.assertEqual((c["calls"], c["wakes"], c["by_kind"]), (3, 1, {"git": c["by_kind"]["git"]}))
            self.assertNotIn("secret", buf.getvalue())


class ScriptTests(unittest.TestCase):
    def test_land_agent_needs_key(self):
        with tempfile.TemporaryDirectory() as d:
            msg = Path(d, "m.txt")
            msg.write_text("x")
            env = {k: v for k, v in os.environ.items() if k != "SUDSY_API_KEY"}
            p = subprocess.run(["bash", str(ROOT / "scripts/land-agent.sh"), d, str(msg)], capture_output=True, text=True, env=env)
            self.assertEqual(p.returncode, 1)
            self.assertIn("BLOCKED: SUDSY_API_KEY is not set", p.stdout)

    def test_land_agent_pushed_line_is_verified(self):
        with tempfile.TemporaryDirectory() as d, fake({"result": "log\nPUSHED abcdef1", "total_cost_usd": 0, "usage": {}}):
            msg = Path(d, "m.txt")
            msg.write_text("x")
            subprocess.run(["git", "init", "-q", d], check=True)
            p = subprocess.run(["bash", str(ROOT / "scripts/land-agent.sh"), d, str(msg)], capture_output=True, text=True)
            self.assertTrue(p.stdout.startswith("BLOCKED"), p.stdout + p.stderr)


if __name__ == "__main__":
    unittest.main()
