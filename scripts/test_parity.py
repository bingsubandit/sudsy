"""Tests of the parity runner (`scripts/parity.sh --test`): pins, filter counts and the `fix` command, on throwaway fixtures."""
import contextlib
import io
import stat
import tempfile
import unittest
import unittest.mock
from collections import Counter
from pathlib import Path

import parity_coverage as pc
import parity_format as pf
import parity_lint as pl
import parity_stability as ps
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
        self.assertIn("target 100.0% (1 of 1); intentional: invalid-input 1, tool-wrong 1, REVIEW 0", out)

    def test_files_of_the_target_are_counted_per_group_without_the_intentional_ones(self):
        with tempfile.TemporaryDirectory() as tmp:
            _, diffs, _ = run_verdicts(self.folder(tmp, "ours"), [("ok.txt", "x", "x"), ("a.txt", "t", "ours"), ("b.txt", None, "o"), ("c.txt", "t", "u")])
        self.assertEqual(diffs.by_group, {"fixtures": [2, 4]})  # a.txt is tool-wrong, b.txt is invalid-input: both leave the target

    def test_group_names_carry_counts_and_a_fixes_note_of_a_removed_name_is_stale(self):
        with tempfile.TemporaryDirectory() as tmp:
            folder = Path(tmp)
            (folder / "differences.yaml").write_text("differences:\n  a: gap  one\n  b: gap  two\nfiles:\n  real-world: a:5, b\nfixes:\n  a: x\n  gone: y\nunclassified: 3\n", encoding="utf-8")
            diffs = pf.Differences(folder)
            diffs.known("real-world", 7)
            with contextlib.redirect_stdout(io.StringIO()) as out:
                bad = diffs.finish()
        self.assertEqual((diffs.files["real-world"], diffs.listed["real-world"], diffs.unclassified), (["a", "b"], 5, 3))
        self.assertEqual(bad, 1)
        self.assertIn("STALE fixes note: gone", out.getvalue())

    def test_changed_output_of_a_pinned_file_is_reviewed_and_counts_against_sudsy(self):
        with tempfile.TemporaryDirectory() as tmp:
            bad, diffs, out = run_verdicts(self.folder(tmp, "ours"), [("ok.txt", "x", "x"), ("a.txt", "t", "changed")])
        self.assertEqual(bad, 1)
        self.assertIn("a.txt: REVIEW", out)
        self.assertIn("target 50.0% (1 of 2); intentional: invalid-input 0, tool-wrong 0, REVIEW 1", out)

    def test_filters_are_counted(self):
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            for name, data in {"a.txt": b"a", "skipped/b.txt": b"b", "c.txt": b"c", "c.opt": b"", "d.txt": b"\xff\xfe"}.items():
                (base / name).parent.mkdir(exist_ok=True)
                (base / name).write_bytes(data)
            filters = Counter()
            found = pf.fixtures({"fixtures": [{"suffixes": [".txt"], "recursive": True, "skip": "skipped/", "skip_with": ".opt"}]}, base, filters)
        self.assertEqual([name for name, _, _, _ in found], ["a.txt"])
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

    def test_coverage_target_compares_lines(self):
        spec = {"format": "regex", "pattern": r"^(?P<file>[^:]+):(?P<line>\d+): (?P<rule>\w+) (?P<message>.*)$"}
        found = pc.parse("a.txt:3: r1 one\nb.txt:5: r2 two\n", spec, {})
        self.assertEqual(found, [("a.txt", "r1", "one", 3), ("b.txt", "r2", "two", 5)])
        # 4 findings of one rule: 2 covered on the line, 1 reviewed as the tool being wrong
        per = {"r1": [4, 2]}
        line = pl.target_line("checkmake", per, 2, 4, 0, "", 1)
        self.assertIn("target 66.7% (2 of 3, same line)", line)
        self.assertIn("tool-wrong 1", line)
        self.assertIn("formatter findings counted as missing 2", pl.target_line("checkmake", per, 2, 4, 2, "", 0))

    def test_a_file_the_tool_rejects_is_invalid_input_without_an_entry_and_a_refuted_one_is_reviewed(self):
        with tempfile.TemporaryDirectory() as tmp:
            folder = Path(tmp)
            (folder / "differences.yaml").write_text("differences:\n  bad: gap  g\nintentional:\n  bad: invalid-input\nfiles:\n  c.txt: bad\n", encoding="utf-8")
            bad, diffs, out = run_verdicts(folder, [("ok.txt", "x", "x"), ("a.txt", None, "o")])
            self.assertEqual(bad, 0)
            self.assertIn("target 100.0% (1 of 1); intentional: invalid-input 1", out)
            diffs.refuted.add("c.txt")  # Sudsy rejects it but the reference parser accepts it
            self.assertEqual(diffs.status("c.txt", None), "REVIEW")

    def test_reviewed_finding_is_pinned_per_finding(self):
        with tempfile.TemporaryDirectory() as tmp:
            folder = Path(tmp)
            (folder / "differences.yaml").write_text("differences:\n  late: gap  the tool reports a line late\nintentional:\n  late: tool-wrong\nfiles:\n  a.mk checkmake:r1 4: late\n", encoding="utf-8")
            reviewed = pf.Differences(folder, lint=True)
            self.assertEqual(reviewed.status("a.mk checkmake:r1 4"), "tool-wrong")
            self.assertIsNone(reviewed.status("a.mk checkmake:r1 5"))


class Missed(unittest.TestCase):
    def check(self, hit, total, text=""):
        with tempfile.TemporaryDirectory() as d, contextlib.redirect_stdout(io.StringIO()):
            folder = Path(d)
            (folder / "differences.yaml").write_text(text)
            per = {"a": [10, 6], "gone": [3, 0]}
            entries = [{"id": "gone", "coverage": "wontfix"}]
            with unittest.mock.patch.object(pl, "catalog_entries", lambda *a, **k: entries):
                return pl.missed_check("t", folder, per, hit, total, Counter(layout=5), Counter(a=1))

    def test_layout_counts_and_wontfix_and_wrong_do_not(self):
        with unittest.mock.patch.object(pl, "catalog_entries", lambda *a, **k: [{"id": "gone", "coverage": "wontfix"}]):
            self.assertEqual(pl.missed_rules("t", {"a": [10, 6], "gone": [3, 0]}, Counter(layout=5), Counter(a=1)), {"a": 3, "layout": 5})

    def test_must_add_up(self):
        full = "measured: {covered: 6, total: 14, main: x}\nmissed:\n  a: 3\n  layout: 5\n"
        self.assertEqual(self.check(6, 14, full), 0)
        self.assertEqual(self.check(6, 15, full), 1)

    def test_a_rule_that_is_not_listed_fails_a_complete_list_only(self):
        self.assertEqual(self.check(6, 14, "measured: {covered: 6, total: 9, main: x}\nmissed:\n  a: 3\n"), 1)
        self.assertEqual(self.check(6, 14, "missed:\n  a: 3\n"), 0)


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


class Stability(unittest.TestCase):
    """`unstable` reads the exit code and the summary line of a fake `sudsy` (a shell script: `$1 format <file>`)."""

    def fake(self, tmp, body):
        exe = Path(tmp) / "sudsy"
        exe.write_text("#!/bin/sh\n" + body, encoding="utf-8")
        exe.chmod(exe.stat().st_mode | stat.S_IEXEC)
        (Path(tmp) / "f.txt").write_text("a\n", encoding="utf-8")
        return str(exe), Path(tmp) / "f.txt"

    def test_each_reason(self):
        cases = [
            ("echo ok >&2", None),
            ("echo 'error: f.txt: 1:1: bad' >&2; exit 2", "does not parse or format"),
            ("echo 'error: f.txt: internal error: changed meaning' >&2; exit 2", "refused by the safety check"),
            ("echo '1 file(s) checked, 0 reformatted, 1 unchanged, 1 copied through' >&2", "copied through"),
            ("echo $RANDOM >> f.txt", "changes on a second format"),
        ]
        for body, why in cases:
            with tempfile.TemporaryDirectory() as tmp:
                exe, file = self.fake(tmp, body)
                self.assertEqual(ps.unstable(exe, file), why, body)


if __name__ == "__main__":
    unittest.main()
