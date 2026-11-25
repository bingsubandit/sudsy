"""Tests of the parity runner (`scripts/parity.sh --test`): pins, filter counts and the `fix` command, on throwaway fixtures."""
import contextlib
import io
import shutil
import stat
import tempfile
import unittest
import unittest.mock
from collections import Counter
from pathlib import Path

import parity_coverage as pc
import parity_gaps as pg
import parity_format as pf
import parity_landing as pld
import parity_lint as pl
import parity_stability as ps
import parity_wrap as pw
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

    def test_a_lint_run_is_stale_only_for_the_files_it_covered(self):
        with tempfile.TemporaryDirectory() as tmp:
            folder = Path(tmp)
            (folder / "differences.yaml").write_text("differences:\n  a: gap  one\nfiles:\n  fix/a tool:x 1: a\n  corpus/b tool:x 2: a\n", encoding="utf-8")
            diffs = pf.Differences(folder, lint=True)
            with contextlib.redirect_stdout(io.StringIO()) as out:
                bad = diffs.finish(scope={"fix/a"})
        self.assertEqual(bad, 1)
        self.assertIn("STALE: fix/a tool:x 1", out.getvalue())
        self.assertNotIn("corpus/b", out.getvalue())

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

    def test_a_file_only_sudsy_rejects_stays_counted_and_a_nul_refusal_is_a_rejection(self):
        err = "error: a.sql: 1:1: syntax error\nerror: n.kt: n.kt is not valid UTF-8 text (or holds a NUL byte)\nerror: z.sql: 2:2: x\n"
        sources = {"a.sql": Path("a.sql"), "n.kt": Path("n.kt"), "z.sql": Path("z.sql")}
        rejected = pc.rejected_files(err, Path("."), sources)
        self.assertEqual(rejected, {"a.sql", "n.kt", "z.sql"})
        found = [("a.sql", "PRS", "m", 1), ("z.sql", "LT01", "m", 1), ("n.kt", "PRS", "m", 1)]
        invalid = pc.invalid_inputs({"parse_error": "PRS"}, sources, rejected, found)
        self.assertEqual(invalid, {"a.sql", "n.kt"})  # z.sql: only Sudsy rejects it, the tool parsed it
        self.assertEqual(pc.invalid_inputs({}, sources, rejected, found), set())

    def test_reviewed_finding_is_pinned_per_finding(self):
        with tempfile.TemporaryDirectory() as tmp:
            folder = Path(tmp)
            (folder / "differences.yaml").write_text("differences:\n  late: gap  the tool reports a line late\nintentional:\n  late: tool-wrong\nfiles:\n  a.mk checkmake:r1 4: late\n", encoding="utf-8")
            reviewed = pf.Differences(folder, lint=True)
            self.assertEqual(reviewed.status("a.mk checkmake:r1 4"), "tool-wrong")
            self.assertIsNone(reviewed.status("a.mk checkmake:r1 5"))


class NotComparable(unittest.TestCase):
    def test_listed_rules_leave_the_findings_and_are_counted(self):
        with tempfile.TemporaryDirectory() as tmp:
            folder = Path(tmp)
            (folder / "differences.yaml").write_text("not-comparable:\n  R2: the tool needs types the corpus lacks\n", encoding="utf-8")
            out = io.StringIO()
            with contextlib.redirect_stdout(out):
                kept = pl.leave_out_not_comparable([("a", "R1", "", 1), ("a", "R2", "", 2), ("b", "R2", "", 3)], pf.Differences(folder, lint=True))
        self.assertEqual(kept, [("a", "R1", "", 1)])
        self.assertIn("not comparable on this corpus 2 findings of 1 of 1 listed rules: R2 2", out.getvalue())


class NamedRules(unittest.TestCase):
    def test_the_parser_diagnostic_is_not_compared_on_names(self):
        ours = {("a", "syntax-error"): [("write `&amp;` for a lone `&`", 1)], ("a", "R"): [("unused `x`", 2)], ("a", "Q"): [("no name", 3)]}
        self.assertEqual(pl.named_rules(ours), {"R"})


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


class CorpusAbsent(unittest.TestCase):
    def test_no_corpus_file_exits_1_and_names_the_fetch_script(self):
        tools = {"lint": {"t": {"fetch": ["tests/conformance/fetch-t-corpus.sh"]}}}
        out = io.StringIO()
        with unittest.mock.patch.object(pf, "tools", lambda: tools), contextlib.redirect_stdout(out):
            pl.require_corpus("t", {"a": 1})
            with self.assertRaises(SystemExit) as e:
                pl.require_corpus("t", {})
        self.assertEqual(e.exception.code, 1)
        self.assertIn("corpus absent", out.getvalue())
        self.assertIn("fetch-t-corpus.sh", out.getvalue())


class StyleLists(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        folder = self.root / "folder"
        folder.mkdir()
        (folder / "s.clang-format").write_text("BasedOnStyle: LLVM\n")
        (folder / "s.yaml").write_text("cpp:\n  format:\n    indent_size: 8\n")
        self.folder = folder
        self.config = {"fetch": ["fetch.sh"], "fixtures": [{"dir": "corpus", "style": "s", "style_list": {"file": ".cf", "key": "Macros", "option": "macros"}}]}

    def tearDown(self):
        self.tmp.cleanup()

    def test_a_list_of_the_corpus_goes_into_both_style_files(self):
        corpus = self.root / "corpus"
        corpus.mkdir()
        (corpus / ".cf").write_text("A: 1\nMacros:\n  - 'x_each'\n  - y_each\nB: 2\n")
        with unittest.mock.patch.object(pf, "ROOT", self.root):
            out = pf.style_files(self.config, self.folder, self.root / "work")
        self.assertEqual((out / "s.clang-format").read_text(), "BasedOnStyle: LLVM\nMacros:\n  - 'x_each'\n  - 'y_each'\n")
        self.assertEqual((out / "s.yaml").read_text(), "cpp:\n  format:\n    macros:\n      - x_each\n      - y_each\n    indent_size: 8\n")

    def test_a_missing_corpus_file_stops_the_run_and_names_the_fetch_script(self):
        with unittest.mock.patch.object(pf, "ROOT", self.root), self.assertRaises(SystemExit) as e:
            pf.style_files(self.config, self.folder, self.root / "work")
        self.assertIn("is missing", str(e.exception))
        self.assertIn("fetch.sh", str(e.exception))


class GroupFiles(unittest.TestCase):
    def test_each_group_gets_its_own_config_file(self):
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            (base / "x.txt").write_text("", encoding="utf-8")
            spec = {"group": "^([^/]+)/", "cwd": "group", "group_files": {"c.neon": "p: x.txt {group}-{prefix}"}, "command": ["cat", "c.neon"],
                    "parse": {"format": "regex", "pattern": "p: (?P<file>\\S+) (?P<rule>.*)"}}
            with unittest.mock.patch.object(pc, "corpus_files", lambda s, f: [("a/x.txt", base / "x.txt"), ("b/x.txt", base / "x.txt")]):
                found, _ = pc.findings("t", spec, "pre", base / "work", "", {})
        self.assertEqual(sorted((f[0], f[1]) for f in found), [("a/x.txt", "a-pre"), ("b/x.txt", "b-pre")])


class ReportParsing(unittest.TestCase):
    def test_a_report_with_no_json_object_is_a_clear_error(self):
        spec = {"format": "json", "json_line": True, "levels": ["files*", "messages"], "file": "0._key", "rule": "1.identifier", "message": "1.message"}
        with self.assertRaises(ValueError):
            pc.parse("Warning: require failed\n", spec, {})


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

WRAP_CFG = {"gap_kinds": ["Whitespace", "Newline", "Continuation"], "comment_kind": "Comment", "eof_kind": "EndOfFile", "string_kind": "String", "brace_kinds": ["Lbrace", "Rbrace"], "macro_boundary": ["Semi", "Lbrace", "Rbrace"], "paren_kinds": ["Lpar", "Rpar"], "format_off": "clang-format off", "format_on": "clang-format on", "boundary": ["statement", "directive"], "container": ["block"], "wrap_inside": []}
WRAP_TAGS = {"Stmt": {"statement"}, "Directive": {"directive"}}
WRAP_TOKENS = [("String", r'(?:u8|u|U|L)?"[^"\n]*"'), ("Comment", r"//[^\n]*"), ("Continuation", r"\\\n"), ("Ident", r"\w+"), ("Semi", ";"), ("Lbrace", "{"), ("Rbrace", "}"), ("Lpar", r"\("), ("Rpar", r"\)"), ("Equal", "="), ("Plus", r"\+"), ("Hash", "#")]


def wrap_tree(src, nodes, bom=False):
    """A Tree of `src` whose depth-1 nodes are `nodes` (kind, first, last token text); the tokens come from a small lexer."""
    import re

    toks = [(m.lastgroup, m.start(), m.end()) for m in re.finditer("|".join(f"(?P<{k}>{r})" for k, r in WRAP_TOKENS), src)]
    lines, i = ["Unit 0.." + str(len(src))], 0
    for kind, first, last in nodes:
        a = next(j for j in range(i, len(toks)) if src[toks[j][1] : toks[j][2]].startswith(first))
        b = min(j for j in range(a, len(toks)) if src[toks[j][1] : toks[j][2]] == last)
        lines.append(f"  {kind} {toks[a][1]}..{toks[b][2]}")
        lines += [f"    {k} {s}..{e} {src[s:e]!r}" for k, s, e in toks[a : b + 1]]
        i = b + 1
    lines.append(f"  EndOfFile {len(src)}..{len(src)} \"\"")
    return pw.Tree(WRAP_CFG, WRAP_TAGS, "\n".join(lines), (b"\xef\xbb\xbf" if bom else b"") + src.encode())


def wrap_equal(theirs, ours, nodes):
    return pw.compare(wrap_tree(theirs, nodes), wrap_tree(ours, nodes))


def block_tree(src):
    """A Tree of `src` (one statement holding a hugging block from its first `{` to its last `}`)."""
    import re

    toks = [(m.lastgroup, m.start(), m.end()) for m in re.finditer("|".join(f"(?P<{k}>{r})" for k, r in WRAP_TOKENS), src)]
    a, b = (next(j for j, t in enumerate(toks) if t[0] == "Lbrace"), max(j for j, t in enumerate(toks) if t[0] == "Rbrace"))
    lines = [f"Unit 0..{len(src)}", f"  Stmt {toks[0][1]}..{toks[-1][2]}"]
    for j, (k, s, e) in enumerate(toks):
        if j == a:
            lines.append(f"    Block {s}..{toks[b][2]}")
        lines.append(f"{'      ' if a <= j <= b else '    '}{k} {s}..{e} {src[s:e]!r}")
    lines.append(f"  EndOfFile {len(src)}..{len(src)} \"\"")
    return pw.Tree(WRAP_CFG, {**WRAP_TAGS, "Block": {"block", "huggable"}}, "\n".join(lines), src.encode())


class Wrap(unittest.TestCase):
    def test_a_control_never_moves_a_block_left_of_column_zero(self):
        src = "x =\n      a + {\n  b;\n};\n"
        tree = block_tree(src)
        self.assertTrue(pw.compare(tree, tree)[0])
        for name, damaged in pw.controls(None, tree, src, ".c", "x").items():
            if isinstance(damaged, pw.Tree):
                self.assertTrue(pw.compare(tree, damaged, tree)[0], name)

    STMT = [("Stmt", "x", ";")]

    def test_a_wrap_point_is_a_space_or_a_newline_at_any_indent(self):
        self.assertTrue(wrap_equal("x = a +\n    b;\n", "x = a + b;\n", self.STMT)[0])
        self.assertTrue(wrap_equal("x = a +\n    b;\n", "x = a +\n  b;\n", self.STMT)[0])

    def test_a_byte_order_mark_is_not_part_of_the_dumped_text(self):
        src = "x = a +\n    b;\n"
        plain, marked = wrap_tree(src, self.STMT), wrap_tree(src, self.STMT, bom=True)
        self.assertEqual([marked.gap(i) for i in range(marked.n)], [plain.gap(i) for i in range(plain.n)])
        self.assertTrue(pw.compare(marked, plain)[0])

    def test_a_changed_token_or_spacing_is_a_difference(self):
        self.assertEqual(wrap_equal("x = a + b;\n", "x = a - b;\n", self.STMT)[1], ["tokens-other"])
        self.assertEqual(wrap_equal("x = a + b;\n", "x =  a + b;\n", self.STMT)[1], ["spacing"])

    def test_every_differing_gap_is_listed_with_its_kinds_and_blanks(self):
        ref, out = wrap_tree("x = a + b;\ny =  c;\n", [("Stmt", "x", ";"), ("Stmt", "y", ";")]), wrap_tree("x = a +  b;\ny = c;\n", [("Stmt", "x", ";"), ("Stmt", "y", ";")])
        found = pw.gaps(ref, out)
        self.assertEqual([(g["c"], g["o"], g["t"]) for g in found], [("spacing", "  ", " "), ("spacing", " ", "  ")])
        self.assertEqual(pw.gaps(wrap_tree("x = a;\n", self.STMT), wrap_tree("x = b;\n", self.STMT))[0]["c"], "tokens")

    def test_statements_stay_on_their_own_lines(self):
        nodes = [("Stmt", "x", ";"), ("Stmt", "y", ";")]
        self.assertEqual(wrap_equal("x = 1;\ny = 2;\n", "x = 1; y = 2;\n", nodes)[1], ["statement-join"])

    def test_the_statements_of_a_macro_body_are_compared(self):
        macro = [("Directive", "#", "0")]
        theirs = "#define F(a) \\\n  do { \\\n    a; \\\n  } while (0)\n"
        self.assertTrue(wrap_equal(theirs, theirs, macro)[0])
        self.assertFalse(wrap_equal(theirs, "#define F(a) \\\n  do { a; \\\n  } while (0)\n", macro)[0])
        self.assertTrue(wrap_equal("#define F(a) a + \\\n  1\n", "#define F(a) a + 1\n", [("Directive", "#", "1")])[0])  # a wrap point of a macro body

    def test_text_under_clang_format_off_is_compared_exactly(self):
        nodes = [("Stmt", "//", ";"), ("Stmt", "y", ";")]
        src = "// clang-format off\nx = a +\n  b;\n// clang-format on\ny = c;\n"
        self.assertTrue(wrap_equal(src, src, nodes)[0])
        self.assertFalse(wrap_equal(src, src.replace("a +\n  b", "a + b"), nodes)[0])


    def literal_score(self, source, theirs, ours, nodes=STMT):
        """Checker.score on texts whose trees come from the test lexer."""
        chk = pw.Checker.__new__(pw.Checker)
        chk.tree = lambda text, suffix: wrap_tree(text, nodes)
        r = chk.score(theirs, ours, ".c", None, source)
        return r["ok"], r["causes"]

    def test_a_string_literal_the_tool_split_equals_the_whole_literal(self):
        one, two = 'x = "abcd";\n', 'x = "ab"\n    "cd";\n'
        self.assertEqual(self.literal_score(one, two, one), (True, []))
        self.assertEqual(self.literal_score('x = L"abcd";\n', 'x = L"ab"\n    L"cd";\n', 'x = L"abcd";\n'), (True, []))
        self.assertEqual(self.literal_score(one, two, 'x = "abXd";\n'), (False, ["tokens-other"]))  # another content
        self.assertEqual(self.literal_score(one, two, 'x = L"abcd";\n'), (False, ["tokens-other"]))  # another prefix
        self.assertEqual(self.literal_score(one, 'x = L"ab"\n    "cd";\n', one), (False, ["tokens-other"]))  # a run of two prefixes

    def test_a_source_of_two_literals_stays_two_unless_the_tool_changed_them(self):
        two, one = 'x = "ab" "cd";\n', 'x = "abcd";\n'
        self.assertEqual(self.literal_score(two, two, one), (False, ["tokens-string-split"]))  # Sudsy joined what the tool kept
        self.assertEqual(self.literal_score(two, 'x = "ab"\n    "cd" "e";\n', 'x = "ab" "cd" "e";\n'), (True, []))
        self.assertEqual(self.literal_score(two, 'x = "a"\n    "b"\n    "cd";\n', two), (True, []))  # the tool split further

    def test_the_string_controls_are_differences(self):
        src = 'x = "abcd";\n'
        tree = wrap_tree(src, self.STMT)
        c = pw.controls(None, tree, src, ".c", "x")
        self.assertEqual(c["damage-string"], 'x = "Xbcd";\n')
        self.assertEqual(c["split-string"], 'x = "ab" "cd";\n')
        self.assertTrue(self.literal_score(src, src, src)[0])
        self.assertFalse(self.literal_score(src, src, c["damage-string"])[0])
        self.assertFalse(self.literal_score(src, src, c["split-string"])[0])  # the source kept one literal: Sudsy splitting is a difference
        directive = 'x = 1;\n'
        self.assertEqual(pw.controls(None, wrap_tree(directive, self.STMT), directive, ".c", "x")["damage-string"], directive)

    def test_no_literal_is_joined_in_a_directive_or_under_clang_format_off(self):
        directive = [("Directive", "#", ";")]
        theirs, ours = '#define X "ab"\n  "cd";\n', '#define X "abcd";\n'
        self.assertEqual(self.literal_score(ours, theirs, ours, directive), (False, ["tokens-other"]))
        off = [("Stmt", "//", ";")]
        theirs = '// clang-format off\nx = "ab"\n  "cd";\n// clang-format on\n'
        ours = '// clang-format off\nx = "abcd";\n// clang-format on\n'
        self.assertEqual(self.literal_score(ours, theirs, ours, off), (False, ["tokens-other"]))


class Gaps(unittest.TestCase):
    def rec(self, c, a, b, o="", t=" "):
        return {"c": c, "a": a, "b": b, "p": "N", "g": "", "o": o, "t": t, "x": ""}

    def test_a_set_makes_a_file_equal_only_when_all_its_causes_are_in_it(self):
        files = {"g/a": [self.rec("spacing", "X", "Y"), self.rec("spacing", "Y", "Z")], "g/b": [self.rec("spacing", "X", "Y")], "g/c": [self.rec("indent", "Y", "Z")]}
        causes = {n: {pg.sig(r, "kinds") for r in recs} for n, recs in files.items()}
        order = pg.greedy({n: set(c) for n, c in causes.items()}, 5)
        self.assertEqual([(new, done) for _, new, done in order], [(1, 1), (1, 2), (1, 3)])
        self.assertEqual(order[0][0][1:3], ("X", "Y"))  # finishes `b` and brings `a` nearer
        self.assertTrue(pg.matches(self.rec("spacing", "X", "Y"), pg.pattern("c=spacing,a=X|W")))
        self.assertFalse(pg.matches(self.rec("indent", "X", "Y"), pg.pattern("c=spacing")))

    def test_a_gap_is_named_by_its_kinds(self):
        self.assertEqual(pg.gap_name(self.rec("spacing", "Rpar", "Ident")), "cast-space")
        self.assertEqual(pg.gap_name(self.rec("spacing", "Ident", "Star")), "pointer-align")
        self.assertEqual(pg.gap_name(self.rec("brace-placement", "Rpar", "Lbrace", "nl1", " ")), "brace-own-line-in-source")


class FetchScripts(unittest.TestCase):
    def test_every_corpus_entry_names_fetch_scripts_that_exist_and_make_its_folder(self):
        root = Path(__file__).resolve().parent.parent
        for mode, entries in pf.tools().items():
            if mode not in ("lint", "format"):
                continue
            for name, entry in entries.items():
                fetch = entry.get("fetch", [])
                for script in fetch:
                    self.assertTrue((root / script).is_file(), f"{mode} {name}: {script} does not exist")
                for source in entry.get("coverage", {}).get("corpus", []):
                    folder = source["dir"].split("tests/conformance/")[-1]
                    if not folder.startswith("corpus/"):
                        continue
                    top = folder.split("/")[1]
                    texts = "".join((root / s).read_text(encoding="utf-8") for s in fetch if (root / s).is_file())
                    self.assertTrue(fetch, f"{mode} {name}: corpus {folder} but no fetch")
                    self.assertIn(top, texts, f"{mode} {name}: no fetch script of the entry makes corpus/{top}")


class Pwsh(unittest.TestCase):
    def test_a_host_without_pwsh_gets_the_pinned_release_on_its_path(self):
        with tempfile.TemporaryDirectory() as tmp:
            calls = []
            def fake_run(cmd, cwd):
                calls.append(cmd[-1])
                (Path(tmp) / "pwsh-7.5.4").mkdir(exist_ok=True)
                (Path(tmp) / "pwsh-7.5.4" / "pwsh").write_text("")
            with unittest.mock.patch.object(pf.shutil, "which", lambda n: None), unittest.mock.patch.object(pf, "run", fake_run), \
                    unittest.mock.patch.object(pf.sys, "platform", "linux"), unittest.mock.patch.dict(pf.os.environ, {"PATH": "/bin"}):
                pf.ensure_pwsh("7.5.4", Path(tmp))
                self.assertTrue(pf.os.environ["PATH"].startswith(str(Path(tmp) / "pwsh-7.5.4")))
            self.assertIn("powershell-7.5.4-linux-", calls[0])


class Landing(unittest.TestCase):
    """The landing check on a throwaway corpus and a fake `sudsy` that upper-cases the files it is given."""

    def setup(self, tmp, sample="keep", corpus_files=("a.txt", "b.txt")):
        tmp = Path(tmp)
        corpus, folder = tmp / "corpus", tmp / "leg"
        corpus.mkdir()
        folder.mkdir()
        (folder / "sudsy.yaml").write_text("", encoding="utf-8")
        for name in corpus_files:
            (corpus / name).write_text(f"{name}\n", encoding="utf-8")
        exe = tmp / "sudsy"
        exe.write_text('#!/bin/sh\nshift\nfor f; do tr a-z A-Z < "$f" > "$f.t"; mv "$f.t" "$f"; done\n', encoding="utf-8")
        exe.chmod(exe.stat().st_mode | stat.S_IEXEC)
        if sample == "keep":
            sample = "".join(f"{pld.digest(n.upper() + chr(10))} {n}\n" for n in ("a.txt", "b.txt"))
        (folder / "landing.txt").write_text(pld.HEADER + sample, encoding="utf-8")
        config = {"fixtures": [{"dir": str(corpus), "suffixes": [".txt"]}]}
        return config, folder, str(exe), tmp / "work", corpus

    def check(self, tmp, ci=False, **kw):
        config, folder, exe, work, corpus = self.setup(tmp, **kw)
        return pld.check("leg", config, folder, exe, work, ci, corpus)

    def test_files_equal_to_the_recorded_hashes_pass(self):
        with tempfile.TemporaryDirectory() as tmp:
            self.assertEqual(self.check(tmp), ([], None))

    def test_a_wrong_hash_fails_and_names_the_file_and_the_way_to_record_a_wanted_difference(self):
        with tempfile.TemporaryDirectory() as tmp:
            problems, _ = self.check(tmp, sample=f"{pld.digest('A.TXT' + chr(10))} a.txt\n{'0' * 16} b.txt\n")
        self.assertEqual(len(problems), 1)
        self.assertIn("1 of 2", problems[0])
        self.assertIn("  b.txt", problems[0])
        self.assertNotIn("  a.txt", problems[0])
        self.assertIn("PARITY_WRITE=1 scripts/parity.sh format leg", problems[0])

    def test_an_empty_or_missing_sample_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            self.assertEqual(len(self.check(tmp, sample="")[0]), 1)
        with tempfile.TemporaryDirectory() as tmp:
            config, folder, exe, work, corpus = self.setup(tmp)
            (folder / "landing.txt").unlink()
            self.assertEqual(len(pld.check("leg", config, folder, exe, work, False, corpus)[0]), 1)

    def test_a_missing_corpus_file_fails_in_ci_and_is_named_when_skipped_elsewhere(self):
        with tempfile.TemporaryDirectory() as tmp:
            problems, note = self.check(tmp, ci=True, corpus_files=("a.txt",))
            self.assertEqual((len(problems), note), (1, None))
            self.assertIn("corpus is absent", problems[0])
        with tempfile.TemporaryDirectory() as tmp:
            problems, note = self.check(tmp, corpus_files=("a.txt",))
            self.assertEqual(problems, [])
            self.assertIn("skipped", note)

    def test_the_sample_is_the_first_files_in_hash_order_and_reads_back(self):
        with tempfile.TemporaryDirectory() as tmp:
            corpus, folder = Path(tmp) / "corpus", Path(tmp)
            equal = {corpus / f"f{i}.txt": f"t{i}\n" for i in range(pld.SAMPLE + 50)}
            with contextlib.redirect_stdout(io.StringIO()):
                pld.write(folder, "leg", equal, corpus)
                pld.write(folder, "leg", {Path(tmp) / "outside.txt": "x"}, corpus)  # not a pinned corpus file: the sample stays
            entries = pld.read(folder)
        self.assertEqual(len(entries), pld.SAMPLE)
        self.assertEqual(entries, sorted(entries, key=lambda e: e[1]))
        self.assertEqual(entries[0][0], pld.digest(equal[corpus / entries[0][1]]))


class LintLanding(unittest.TestCase):
    """The lint landing check on a throwaway corpus and a fake `sudsy` that reports `r1` on line 3 of a.txt."""

    REPORT = '{"diagnostics":[{"path":"a.txt","rule":"r1","range":{"start":{"line":3}}},{"path":"b.txt","rule":"other","range":{"start":{"line":1}}}]}'

    def check(self, tmp, sample, files=("a.txt", "b.txt"), report=REPORT, ci=False):
        tmp = Path(tmp)
        corpus, folder = tmp / "corpus", tmp / "leg"
        corpus.mkdir()
        folder.mkdir()
        for name in files:
            (corpus / name).write_text("x\n", encoding="utf-8")
        exe = tmp / "sudsy"
        exe.write_text(f"#!/bin/sh\ncat <<'EOF'\n{report}\nEOF\n", encoding="utf-8")
        exe.chmod(exe.stat().st_mode | stat.S_IEXEC)
        if sample is not None:
            (folder / pld.LINT_FILE).write_text(pld.LINT_HEADER + sample, encoding="utf-8")
        names = {corpus / n: n for n in ("a.txt", "b.txt")}
        with unittest.mock.patch.multiple(pl, work_names=lambda t, s: names, sudsy_rules=lambda t: ({"r1"}, {}), sudsy_config=lambda t, s: {}):
            return pld.check_lint("leg", {}, folder, str(exe), tmp / "work", ci, corpus)

    GOOD = f"{pld.lint_digest({('r1', 3)})} a.txt\n{pld.lint_digest(set())} b.txt\n"

    def test_the_recorded_findings_of_the_mapped_rules_pass(self):
        with tempfile.TemporaryDirectory() as tmp:
            self.assertEqual(self.check(tmp, self.GOOD), ([], None))

    def test_a_wrong_hash_fails_and_names_the_file_and_the_way_to_record_it(self):
        with tempfile.TemporaryDirectory() as tmp:
            problems, _ = self.check(tmp, f"{'0' * 16} a.txt\n{pld.lint_digest(set())} b.txt\n")
        self.assertEqual(len(problems), 1)
        self.assertIn("1 of 2", problems[0])
        self.assertIn("  a.txt", problems[0])
        self.assertIn("COVERAGE_WRITE=1 scripts/parity.sh coverage leg", problems[0])

    def test_a_rule_turned_off_and_a_rule_reporting_more_both_fail(self):
        off = '{"diagnostics":[]}'
        more = '{"diagnostics":[{"path":"a.txt","rule":"r1","range":{"start":{"line":3}}},{"path":"a.txt","rule":"r1","range":{"start":{"line":9}}}]}'
        for report in (off, more):
            with tempfile.TemporaryDirectory() as tmp:
                problems, _ = self.check(tmp, self.GOOD, report=report)
            self.assertEqual(len(problems), 1, report)

    def test_an_empty_or_missing_sample_fails(self):
        for sample in ("", None):
            with tempfile.TemporaryDirectory() as tmp:
                self.assertEqual(len(self.check(tmp, sample)[0]), 1)

    def test_a_missing_corpus_file_fails_in_ci_and_is_skipped_elsewhere(self):
        with tempfile.TemporaryDirectory() as tmp:
            problems, note = self.check(tmp, self.GOOD, files=("a.txt",), ci=True)
            self.assertEqual((len(problems), note), (1, None))
        with tempfile.TemporaryDirectory() as tmp:
            problems, note = self.check(tmp, self.GOOD, files=("a.txt",))
            self.assertEqual(problems, [])
            self.assertIn("skipped", note)

    def test_the_selection_takes_files_per_rule_zero_finding_files_and_prefers_agreeing_ones(self):
        states = {f"z{i}.txt": set() for i in range(200)}
        states.update({f"r{i}.txt": {(f"rule{i % 4}", 1)} for i in range(40)})
        chosen, filled = pld.select_lint(states, set(states))
        self.assertFalse(filled)
        self.assertEqual(len(chosen), pld.LINT_FILES)
        for rule in range(4):
            self.assertGreaterEqual(sum((f"rule{rule}", 1) in states[p] for p in chosen), pld.LINT_PER_RULE)
        self.assertGreaterEqual(sum(not states[p] for p in chosen), round(pld.LINT_FILES * pld.LINT_ZERO))
        self.assertEqual(chosen, pld.select_lint(states, set(states))[0])  # deterministic
        few, filled = pld.select_lint(states, {"r0.txt"})
        self.assertTrue(filled)
        self.assertIn("r0.txt", few)


class BatchRun(unittest.TestCase):
    """`input: batch` of a slow tool: config `files`, the `cache` of its output and the `regressions` count."""

    def config(self, tool, **more):
        return {"catalog": "unit-batch", "install": {"exe": tool}, "command": ["{exe}"], "args": ["a.txt"], **more}

    def tool(self, tmp, body):
        prefix = Path(tmp) / "prefix"
        prefix.mkdir()
        exe = prefix / "tool"
        exe.write_text("#!/bin/sh\n" + body, encoding="utf-8")
        exe.chmod(exe.stat().st_mode | stat.S_IEXEC)
        fixture = Path(tmp) / "a.txt"
        fixture.write_text("in\n", encoding="utf-8")
        return prefix, [("a.txt", fixture, None, None)]

    def test_config_files_are_written_next_to_the_copies_with_the_prefix(self):
        with tempfile.TemporaryDirectory() as tmp:
            prefix, found = self.tool(tmp, 'cat conf.txt > a.txt\n')
            work = Path(tmp) / "work"
            work.mkdir()
            out = pf.upstream_batch(self.config("tool", files={"conf.txt": "root {prefix}\n"}), prefix, found, work)
            self.assertEqual(out["a.txt"], f"root {prefix}\n")

    def test_cache_keeps_the_changed_files_and_is_keyed_by_the_fixture_contents(self):
        with tempfile.TemporaryDirectory() as tmp:
            prefix, found = self.tool(tmp, 'echo changed > a.txt\necho run >> "$COUNT"\n')
            count = Path(tmp) / "count"
            config = self.config("tool", cache=True)
            kept = []
            try:
                with unittest.mock.patch.dict("os.environ", {"COUNT": str(count)}):
                    for n in range(2):
                        work = Path(tmp) / f"work{n}"
                        work.mkdir()
                        self.assertEqual(pf.upstream_batch(config, prefix, found, work)["a.txt"], "changed\n")
                    self.assertEqual(count.read_text().count("run"), 1)  # the second call came from the cache
                    found[0][1].write_text("other\n", encoding="utf-8")
                    work = Path(tmp) / "work2"
                    work.mkdir()
                    pf.upstream_batch(config, prefix, found, work)
                    self.assertEqual(count.read_text().count("run"), 2)  # other contents, other key
            finally:
                kept = list((pf.ROOT / ".cargo/tmp/rb").glob("unit-batch-*"))
                for d in kept:
                    shutil.rmtree(d)
            self.assertEqual(len(kept), 2)

    def test_regressions_count_the_files_the_tool_leaves_alone_that_sudsy_changes(self):
        with tempfile.TemporaryDirectory() as tmp:
            a, b, c = (Path(tmp) / n for n in "abc")
            for p in (a, b, c):
                p.write_text("x\n", encoding="utf-8")
            found = [("a", a, "g", None), ("b", b, "g", None), ("c", c, None, None)]
            results = [("x\n", "y\n", None, None), ("z\n", "y\n", None, None), ("x\n", "x\n", None, None)]
            self.assertEqual(dict(pf.regression_counts(found, results)), {"g": 1, "fixtures": 0})


if __name__ == "__main__":
    unittest.main()
