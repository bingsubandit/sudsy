"""score.py RUNDIR: print one JSON line of metrics for a run dir."""
import ast
import collections
import importlib.machinery
import importlib.util
import json
import os
import pathlib
import re
import subprocess
import sys
import tempfile

HERE = pathlib.Path(__file__).resolve().parent
COMMON = HERE.parent / "tools" / "common"
RUN = pathlib.Path(sys.argv[1]).resolve()
EXP, COND, REP = RUN.name.split("-", 2)
ENV = {**os.environ, "PYTHONDONTWRITEBYTECODE": "1"}

SCOPE = {
    "E1": {"shop/inventory.py"},
    "E11": {"shop/inventory.py"},
    "E2": {"shop/users.py"},
    "E3": {"shop/pricing.py"},
    "E4": {"shop/cache.py", "shop/orders.py", "shop/inventory.py", "shop/pricing.py", "shop/users.py"},
    "E5": {"shop/orders.py"},
    "E6": {"shop/pricing.py"},
    "E7": None,  # whole repo is in scope; only tests/ and tools are outside
    "E8": None,
    "E9": {"rules/*"},
    "E10": {"shop/report.py", "shop/cache.py"},
}
TASK_TEST = {"E1": "test_e1", "E11": "test_e1", "E2": "test_e2", "E5": "test_e5", "E6": "test_e6",
             "E7": "test_e7", "E8": "test_legacy", "E10": "test_e10"}


def sh(cmd, cwd=RUN, env=None, timeout=180, **kw):
    try:
        return subprocess.run(cmd, cwd=cwd, env=env or ENV, capture_output=True, text=True, timeout=timeout, **kw)
    except subprocess.TimeoutExpired:
        return subprocess.CompletedProcess(cmd, 124, "", "timeout")


def git(*a):
    return sh(["git", *a]).stdout


def visible(p):
    return not p.startswith(".") and p != "TASK.md" and "__pycache__" not in p


# ---- tool calls
calls = collections.Counter()
stamps = []
try:
    for ln in (RUN / ".calls.log").read_text().splitlines():
        parts = ln.split(None, 2)
        if len(parts) >= 2:
            calls[parts[1]] += 1
            stamps.append(int(parts[0]))
except Exception:  # noqa
    pass

# ---- changes
changed = sorted({p for p in git("diff", "--name-only", "exp-base").splitlines() + git("ls-files", "-o", "--exclude-standard").splitlines() if visible(p)})
ins = dele = 0
for ln in git("diff", "--numstat", "exp-base").splitlines():
    a, b, p = ln.split("\t", 2)
    if visible(p) and a != "-":
        ins += int(a)
        dele += int(b)
scope = SCOPE.get(EXP)
if scope is None:
    outside = [p for p in changed if p.startswith("tests/") or "/" not in p]
else:
    outside = [p for p in changed if not any(p == s or (s.endswith("*") and p.startswith(s[:-1])) for s in scope)]

# ---- tests
t_all = sh(["python3", "-m", "tests"])
res = {
    "exp": EXP, "cond": COND, "rep": REP,
    "tool_calls_by_name": dict(calls), "tool_calls_total": sum(calls.values()),
    "mock_call_span_s": (max(stamps) - min(stamps)) if stamps else 0,
    "files_changed": changed, "files_changed_outside_scope": outside,
    "diff_insertions": ins, "diff_deletions": dele,
    "tests_pass": t_all.returncode == 0,
}
if EXP in TASK_TEST:
    t = sh(["python3", "-m", "tests", TASK_TEST[EXP]])
    res["task_tests_pass"] = t.returncode == 0
    res["base_tests_pass"] = sh(["python3", "-m", "tests", *sorted(p.stem for p in (RUN / "tests").glob("test_*.py") if p.stem != TASK_TEST[EXP]), ]).returncode == 0


def base_dir():
    d = tempfile.mkdtemp(prefix="expbase-")
    subprocess.run(f"git archive exp-base | tar -x -C {d}", shell=True, cwd=RUN, check=True)
    return pathlib.Path(d)


def load_lintb():
    ld = importlib.machinery.SourceFileLoader("lintb", str(COMMON / "lintb"))
    spec = importlib.util.spec_from_loader("lintb", ld)
    m = importlib.util.module_from_spec(spec)
    ld.exec_module(m)
    return m


def ruff_default(path, cwd):
    r = sh(["ruff", "check", "--output-format", "concise", "-q", "--no-cache", path], cwd=cwd)
    return [ln for ln in r.stdout.splitlines() if ln.strip()]


def r123(args, cwd=RUN, **env):
    r = sh(["python3", str(COMMON / "r123"), "lint", *args], cwd=cwd, env={**ENV, **env})
    rules = collections.Counter(re.findall(r"^\S+:\d+:\d+ (R\d)", r.stdout, re.M))
    return dict(rules)


if EXP in ("E1", "E11"):
    f = "shop/inventory.py"
    res["formatted"] = sh(["black", "--check", "-q", f]).returncode == 0
    b = base_dir()
    strip = lambda ls: collections.Counter(re.sub(r":\d+:\d+:", ":", x) for x in ls)
    now = strip(ruff_default(f, RUN))
    old = strip(ruff_default(f, b))
    res["ruff_findings_now"] = sum(now.values())
    res["ruff_new_findings"] = sum((now - old).values())
    res["lint_clean"] = res["ruff_new_findings"] == 0
    res["function_defined"] = "def reorder_list" in (RUN / f).read_text()
if EXP == "E2":
    lb = load_lintb()
    b = base_dir()
    cwd = os.getcwd()
    os.chdir(b)
    old = collections.Counter(r[:3] for r in lb.current())
    os.chdir(RUN)
    lb.LINE_CACHE.clear()
    now = collections.Counter(r[:3] for r in lb.current())
    os.chdir(cwd)
    res["new_findings"] = sum((now - old).values())
    res["preexisting_findings_fixed"] = sum((old - now).values())
    res["lint_clean"] = res["new_findings"] == 0
if EXP == "E3":
    left = r123(["shop/pricing.py"])
    res["lint_clean"] = not left
    res["remaining_by_rule"] = left
if EXP == "E6":
    left = r123(["shop"])
    res["lint_clean"] = not left
    res["remaining_by_rule"] = left
    src = (RUN / "shop/pricing.py").read_text()
    res["refund_defined"] = "def refund" in src
    res["uses_print"] = "print(" in src
if EXP == "E8":
    left = r123(["shop"], R2_SCOPE="all")
    b = base_dir()
    old = r123(["shop"], cwd=b, R2_SCOPE="all")
    res["remaining_by_rule"] = left
    res["base_by_rule"] = old
    res["r2_r3_clean"] = not left.get("R2") and not left.get("R3")
    res["r1_scope_creep"] = old.get("R1", 0) - left.get("R1", 0)
    res["lint_clean"] = res["r2_r3_clean"]
if EXP == "E7":
    fmt = sh(["black", "--check", "-q", "shop"]).returncode == 0
    lint = sh(["ruff", "check", "-q", "--no-cache", "shop"]).returncode == 0
    typ = sh(["python3", str(COMMON / "typecheck_impl")]).returncode == 0
    res.update(fmt_ok=fmt, lint_ok=lint, typecheck_ok=typ, lint_clean=fmt and lint and typ)
if EXP == "E9":
    r = sh(["python3", str(COMMON / "rules_lint")])
    res["rule_files"] = sorted(p.name for p in (RUN / "rules").glob("*.yaml") if p.name != "example.yaml")
    res["rule_file_valid"] = r.returncode != 2
    src = (RUN / "shop/clock.py").read_text().splitlines()
    hit = {int(m.group(1)) for m in re.finditer(r"^shop/clock\.py:(\d+):", r.stdout, re.M)}
    bad = {i + 1 for i, ln in enumerate(src) if re.search(r"datetime\.now\(\s*\)", ln) and "#" not in ln and '"' not in ln}
    good = {i + 1 for i in range(len(src))} - bad
    other = [ln for ln in r.stdout.splitlines() if not ln.startswith("shop/clock.py:") and ln.strip()]
    res["bad_sites"] = len(bad)
    res["bad_flagged"] = len(bad & hit)
    res["false_positives_in_clock"] = len({i + 1 for i, ln in enumerate(src) if ("datetime.now(" in ln) and (i + 1) not in bad} & hit)
    res["findings_elsewhere"] = len(other)
    res["rule_ok"] = bool(res["rule_files"]) and res["bad_flagged"] == len(bad) and res["false_positives_in_clock"] == 0 and not other and res["rule_file_valid"]
    res["lint_clean"] = res["rule_ok"]
if EXP == "E10":
    rep = (RUN / "shop/report.py").read_text()
    cache = (RUN / "shop/cache.py").read_text()
    res["uses_get_many"] = "get_many" in rep
    res["get_many_defined"] = "def get_many" in cache
    res["lint_clean"] = bool(res["task_tests_pass"])
if EXP == "E5":
    try:
        tree = ast.parse((RUN / "shop/orders.py").read_text())
        meth = [n for c in tree.body if isinstance(c, ast.ClassDef) and c.name == "OrderBook" for n in c.body
                if isinstance(n, ast.FunctionDef) and n.name == "total_with_tax"]
        src = ast.unparse(meth[0]) if meth else ""
    except SyntaxError:
        src = ""
    res["method_defined"] = bool(src)
    res["uses_helpers"] = {h: (h in src) for h in ("_round_money", "_apply_discount", "_tax_for")}
    res["uses_other_rounding"] = bool(re.search(r"\bround\(|quantize|discounted_total|rounded_subtotal", src))
if EXP == "E4":
    b = base_dir()
    sg = sh(["python3", str(COMMON / "sgrep"), "Cache.get"], cwd=b).stdout.splitlines()
    real = collections.defaultdict(collections.Counter)  # file -> stripped old line
    for ln in sg:
        m = re.match(r"([^:]+):(\d+): (.*)$", ln)
        real[m.group(1)][m.group(3)] += 1
    nreal = sum(sum(c.values()) for c in real.values())

    def lines_of(root, rel):
        try:
            return collections.Counter(x.strip() for x in (root / rel).read_text().splitlines())
        except OSError:
            return collections.Counter()

    decoys = renamed = 0
    broken = []
    for p in sorted((b / "shop").glob("*.py")):
        rel = f"shop/{p.name}"
        base_c, cur = lines_of(b, rel), lines_of(RUN, rel)
        for line, n in base_c.items():
            if "get" not in line:
                continue
            r = real[rel].get(line, 0)
            if r:
                new = line.replace("def get(", "def fetch(", 1) if line.startswith("def get(") else line.replace(".get(", ".fetch(", 1)
                renamed += min(r, cur.get(new, 0))
            keep = n - r
            if keep > 0:
                decoys += keep
                if cur.get(line, 0) < keep:
                    broken.append(f"{rel}: {line}")
    # decoy lines that are textually identical to real lines are covered by the real count
    res["real_sites"] = nreal
    res["real_renamed"] = renamed
    res["decoys"] = decoys
    res["decoys_broken"] = broken
    left = [ln for ln in sh(["python3", str(COMMON / "sgrep"), "Cache.get"]).stdout.splitlines()]
    res["real_get_left"] = len(left)
    res["rename_correct"] = renamed == nreal and not broken and not left and res["tests_pass"]
    res["lint_clean"] = res["rename_correct"]

print(json.dumps(res))
