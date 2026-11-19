"""Tests of the parity runner (`scripts/parity.sh --test`): pins, filter counts and the `fix` command, on throwaway fixtures."""
import contextlib
import io
import stat
import tempfile
import unittest
from collections import Counter
from pathlib import Path

import parity_format as pf
import speed

YAML = """differences:
  wrong: choice  the tool is wrong
  bad-input: choice  the file is not valid
intentional:
  wrong: tool-wrong
  bad-input: invalid-input
files:
  a.txt: wrong
  b.txt: bad-input
pins:
  a.txt: {pin}
"""


def run_verdicts(folder, cases):
    diffs = pf.Differences(folder)
    out = io.StringIO()
    with contextlib.redirect_stdout(out):
        bad = sum(pf.verdict(name, theirs, ours, diffs, "t") for name, theirs, ours in cases)
        diffs.summary()
    return bad, diffs, out.getvalue()


class Runner(unittest.TestCase):
    def folder(self, tmp, pinned):
        folder = Path(tmp)
        (folder / "differences.yaml").write_text(YAML.format(pin=pf.pin(pinned)), encoding="utf-8")
        return folder

    def test_pinned_output_counts_as_intentional(self):
        with tempfile.TemporaryDirectory() as tmp:
            bad, diffs, out = run_verdicts(self.folder(tmp, "ours"), [("ok.txt", "x", "x"), ("a.txt", "t", "ours"), ("b.txt", None, "o")])
        self.assertEqual(bad, 0)
        self.assertIn("raw 33.3% (1 of 3); target 100.0% (1 of 1); intentional: invalid-input 1, tool-wrong 1, REVIEW 0", out)

    def test_changed_output_of_a_pinned_file_is_reviewed_and_counts_against_sudsy(self):
        with tempfile.TemporaryDirectory() as tmp:
            bad, diffs, out = run_verdicts(self.folder(tmp, "ours"), [("ok.txt", "x", "x"), ("a.txt", "t", "changed")])
        self.assertEqual(bad, 1)
        self.assertIn("a.txt: REVIEW", out)
        self.assertIn("raw 50.0% (1 of 2); target 50.0% (1 of 2); intentional: invalid-input 0, tool-wrong 0, REVIEW 1", out)

    def test_filters_are_counted(self):
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            for name, data in {"a.txt": b"a", "skipped/b.txt": b"b", "c.txt": b"c", "c.opt": b"", "d.txt": b"\xff\xfe"}.items():
                (base / name).parent.mkdir(exist_ok=True)
                (base / name).write_bytes(data)
            filters = Counter()
            found = pf.fixtures({"fixtures": [{"suffixes": [".txt"], "recursive": True, "skip": "skipped/", "skip_with": ".opt"}]}, base, filters)
        self.assertEqual([name for name, _, _ in found], ["a.txt"])
        self.assertEqual(dict(filters), {"skip": 1, "skip_with": 1, "non-UTF-8 files": 1})

    def test_fix_command_is_compared(self):
        with tempfile.TemporaryDirectory() as tmp:
            fake = Path(tmp) / "sudsy"
            fake.write_text("#!/usr/bin/env python3\nimport sys\nif sys.argv[1] == 'fix':\n    open(sys.argv[2], 'w').write('fixed')\n", encoding="utf-8")
            fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
            file = Path(tmp) / "f.txt"
            file.write_text("input", encoding="utf-8")
            self.assertEqual(pf.sudsy_format(fake, file, Path(tmp), "fix")[0], "fixed")
            file.write_text("input", encoding="utf-8")
            self.assertEqual(pf.sudsy_format(fake, file, Path(tmp), "format")[0], "input")
        self.assertEqual(pf.tools()["format"]["terraform"]["sudsy_command"], "fix")  # the run compares the tool with `sudsy fix`


class SpeedRuns(unittest.TestCase):
    def test_a_sudsy_run_that_checks_no_file_fails_the_row(self):
        with self.assertRaises(SystemExit) as e:
            speed.check_sudsy_run(["sudsy", "lint", "."], "c", 0, "0 file(s) checked, 0 error(s), 0 warning(s)\n")
        self.assertIn("checked 0 files", str(e.exception))
        with self.assertRaises(SystemExit):
            speed.check_sudsy_run(["sudsy", "lint", "."], "c", 0, "")

    def test_a_crash_or_an_error_exit_fails_the_row(self):
        for code in (-11, 2):
            with self.assertRaises(SystemExit):
                speed.check_sudsy_run(["sudsy", "lint", "."], "c", code, "5 file(s) checked, 0 error(s), 0 warning(s)\n")

    def test_findings_and_differences_are_timings(self):
        speed.check_sudsy_run(["sudsy", "format", "--check", "."], "c", 1, "5 file(s) checked, 2 would reformat, 3 unchanged\n")
        self.assertEqual(speed.files_checked("x\n12 file(s) checked, 0 error(s), 0 warning(s)\n"), 12)


if __name__ == "__main__":
    unittest.main()
