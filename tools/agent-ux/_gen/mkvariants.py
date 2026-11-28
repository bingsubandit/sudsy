"""Write tools/variants, tasks and extras for every experiment (idempotent)."""
import os
import pathlib
import shutil
import stat

ROOT = pathlib.Path(__file__).resolve().parent.parent
V = ROOT / "tools" / "variants"
T = ROOT / "tasks"
X = ROOT / "extras"
shutil.rmtree(V, ignore_errors=True)
shutil.rmtree(T, ignore_errors=True)
shutil.rmtree(X, ignore_errors=True)

HDR = '#!/usr/bin/env bash\nR="$(cd "$(dirname "$0")" && pwd)"; cd "$R"\necho "$(date +%s) $(basename "$0") $*" >> "$R/.calls.log"\n'


def w(path, text, exe=False):
    p = pathlib.Path(path)
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(text, newline="\n")
    if exe:
        p.chmod(p.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)


def variant(exp, cond, tools="", needs=(), files=None, setup=None, note=None, settings=None):
    d = V / exp / cond
    d.mkdir(parents=True, exist_ok=True)
    w(d / "TASK_TOOLS.md", tools.strip() + "\n")
    if needs:
        w(d / "NEEDS", "\n".join(needs) + "\n")
    for name, text in (files or {}).items():
        w(d / name, text, exe=not name.endswith((".md", ".json", ".yaml")))
    if setup:
        w(d / "SETUP", setup)
    if note:
        w(d / "TASK_NOTE.md", note)
    if settings:
        w(d / ".claude-settings.json", settings)


def task(exp, text):
    w(T / exp / "TASK.md", text.strip() + "\n")


def extra(exp, name, text):
    w(X / exp / name, text)


# ---------------------------------------------------------------- E1
task("E1", """
# Task
In `shop/inventory.py` add a function `reorder_list(items, threshold)`.
`items` is a list of `shop.models.Item`. Return the skus of all items whose `qty` is
below `threshold`, sorted by `qty` ascending, ties by `sku` ascending. An empty input
gives an empty list.

Project style: code is formatted with `black` (default settings) and new code has no
ruff findings (default ruff rules). `tests/test_e1.py` checks your function.
""")
extra("E1", "tests/test_e1.py", '''from decimal import Decimal

from shop.inventory import reorder_list
from shop.models import Item


def test_reorder():
    items = [
        Item("b", "B", 2, Decimal(1)),
        Item("a", "A", 2, Decimal(1)),
        Item("c", "C", 9, Decimal(1)),
        Item("d", "D", 0, Decimal(1)),
    ]
    assert reorder_list(items, 5) == ["d", "a", "b"]
    assert reorder_list(items, 0) == []
    assert reorder_list([], 3) == []
''')
check_a = HDR + 'f="${1:?usage: ./check FILE}"\nblack -q "$f"\nout="$(ruff check --output-format concise -q "$f")"\n[ -n "$out" ] && echo "$out" || echo ok\n'
check_b = HDR + ('f="${1:?usage: ./check FILE}"\ntmp="$(mktemp)"; cp "$f" "$tmp"\nblack -q "$f"\n'
                 'if ! cmp -s "$tmp" "$f"; then echo "black reformatted $f:"; diff -U1 "$tmp" "$f" | tail -n +3; fi\nrm -f "$tmp"\n'
                 'out="$(ruff check --output-format concise -q "$f")"\n[ -n "$out" ] && echo "$out" || echo ok\n')
check_c = HDR + 'f="${1:?usage: ./check FILE}"\nout="$(ruff check --output-format concise -q "$f")"\n[ -n "$out" ] && echo "$out" || echo ok\n'
variant("E1", "A", "`./check FILE` formats FILE in place with black, then prints ruff findings for it, or `ok`.", files={"check": check_a})
variant("E1", "B", "`./check FILE` formats FILE in place with black and prints the diff black applied, then prints ruff findings for it, or `ok`.", files={"check": check_b})
variant("E1", "C", "`./check FILE` prints ruff findings for FILE, or `ok`. It does not format anything.", files={"check": check_c})

POST = '''#!/usr/bin/env bash
R="$(cd "$(dirname "$0")/.." && pwd)"
f="$(python3 -c 'import sys,json;print(json.load(sys.stdin).get("tool_input",{}).get("file_path",""))')"
case "$f" in *.py) ;; *) exit 0;; esac
cd "$R"
echo "$(date +%s) hook-post $f" >> "$R/.calls.log"
@@BLACK@@
out="$(ruff check --output-format concise -q "$f")"
if [ -n "$out" ]; then echo "$out" >&2; exit 2; fi
exit 0
'''
STOP = '''#!/usr/bin/env bash
R="$(cd "$(dirname "$0")/.." && pwd)"; cd "$R"
echo "$(date +%s) hook-stop shop/inventory.py" >> "$R/.calls.log"
black -q shop/inventory.py
exit 0
'''
SET_D = '{"hooks": {"PostToolUse": [{"matcher": "Edit|Write|MultiEdit", "hooks": [{"type": "command", "command": "__RUN__/.mock/posthook"}]}]}}\n'
SET_E = '{"hooks": {"PostToolUse": [{"matcher": "Edit|Write|MultiEdit", "hooks": [{"type": "command", "command": "__RUN__/.mock/posthook"}]}], "Stop": [{"hooks": [{"type": "command", "command": "__RUN__/.mock/stophook"}]}]}}\n'
TOOLS_D = "There is no check script. After every edit of a Python file an automatic hook formats it with black and reports ruff findings back to you (nothing is reported when the file is clean)."
TOOLS_E = "There is no check script. After every edit of a Python file an automatic hook reports ruff findings back to you (nothing is reported when clean); black formatting is applied automatically when you finish."
variant("E1", "D", TOOLS_D, files={".mock/posthook": POST.replace('@@BLACK@@', 'black -q "$f"')}, settings=SET_D)
variant("E1", "E", TOOLS_E, files={".mock/posthook": POST.replace("@@BLACK@@", ":"), ".mock/stophook": STOP}, settings=SET_E)

# E11: E1/D with and without the "never format" sentence
SENT = "Formatting is automatic; never spend effort on whitespace, quotes or line length.\n"
task("E11", (T / "E1" / "TASK.md").read_text())
shutil.copytree(X / "E1", X / "E11")
variant("E11", "A", TOOLS_D, files={".mock/posthook": POST.replace('@@BLACK@@', 'black -q "$f"')}, settings=SET_D)
variant("E11", "B", TOOLS_D, files={".mock/posthook": POST.replace('@@BLACK@@', 'black -q "$f"')}, settings=SET_D, note=SENT)

# ---------------------------------------------------------------- E2
task("E2", """
# Task
In `shop/users.py` add a function `active_users(users)`. `users` is a list of
`shop.models.User`; return those with `active` true, sorted by `name` case-insensitively.

Your change must not introduce new lint findings (project lint: `./lint`). Findings that
already existed are not your job: do not fix them. `tests/test_e2.py` checks your function.
""")
extra("E2", "tests/test_e2.py", '''from shop.models import User
from shop.users import active_users


def test_active():
    us = [User(1, "bob"), User(2, "Ada"), User(3, "Cleo", False), User(4, "dave")]
    assert [u.name for u in active_users(us)] == ["Ada", "bob", "dave"]
    assert active_users([]) == []
''')
RUFFALL = 'ruff check --select ALL --ignore D --output-format concise --no-cache .'
variant("E2", "A", "`./lint` prints all ruff findings of the whole repository (`path:line:col: CODE message`).",
        files={"lint": HDR + RUFFALL + "\n"})
variant("E2", "B", "`./lint` prints only ruff findings that did not exist when you started (the existing ones are filtered out). Silent with exit 0 means no new findings.",
        needs=["lintb"], files={"lint": HDR + 'exec python3 .mock/lintb\n'}, setup='python3 .mock/lintb --baseline\n')

# ---------------------------------------------------------------- E3
task("E3", """
# Task
`./lint shop/pricing.py` reports violations of the project rules in that file. Fix the
code so that it reports nothing (silent, exit 0), without changing behavior:
`tests/test_pricing.py` must still pass.
""")
r123 = lambda style, extra_env="": HDR + f'STYLE={style} {extra_env}exec python3 .mock/r123 lint "$@"\n'
variant("E3", "A", "`./lint FILE` prints violations as `path:line:col RULE short-message`; silent with exit 0 means clean.",
        needs=["r123"], files={"lint": r123("terse")})
variant("E3", "B", "`./lint FILE` prints violations as `path:line:col RULE message`, followed by the offending source line and a `fix:` line with the replacement text; silent with exit 0 means clean.",
        needs=["r123"], files={"lint": r123("verbose")})
variant("E3", "C", "`./lint FILE` prints violations as `path:line:col RULE short-message [id XXXX]`; silent with exit 0 means clean. `./fix XXXX` applies the fix of one finding to the file (imports included).",
        needs=["r123"], files={"lint": r123("id"), "fix": HDR + 'exec python3 .mock/r123 fix "$@"\n'})

# ---------------------------------------------------------------- E4
task("E4", """
# Task
Rename the method `Cache.get` (defined in `shop/cache.py`) to `Cache.fetch`. Update the
definition and every call site of that method. Rename nothing else: other functions,
methods or dict lookups that happen to be called `get` must stay as they are. All tests
must still pass.
""")
variant("E4", "A", "No special tools; use grep and the other standard tools.")
variant("E4", "B", "`./sgrep Cache.get` prints `path:line: source line` for the definition and exactly the real call sites of `Cache.get` (it ignores unrelated `get`s). After the rename the same tool takes `Cache.fetch`.",
        needs=["sgrep"], files={"sgrep": HDR + 'exec python3 .mock/sgrep "$@"\n'})

# ---------------------------------------------------------------- E5
task("E5", """
# Task
In `shop/orders.py` add the method `OrderBook.total_with_tax(self, region)`. It returns the
book's subtotal after the book's own discount (`self.discount_pct`), plus the sales tax of
`region`, as a Decimal rounded to cents once, at the end (no rounding of intermediate
values). An unknown region raises `ValueError`. Use the existing helpers in the module
(`_round_money`, `_apply_discount`, `_tax_for`) instead of re-implementing them.
`tests/test_e5.py` checks it.
""")
extra("E5", "tests/test_e5.py", '''from decimal import Decimal

from shop.orders import LineItem, Order, OrderBook


def make_book():
    book = OrderBook()
    book.add(Order("o1", [LineItem("A", 2, Decimal("10.01")), LineItem("X", 1, Decimal("5.005"))]))
    book.add(Order("o2", [LineItem("Y", 3, Decimal("99.99"))]))
    return book


def test_total_with_tax():
    book = make_book()
    assert book.total_with_tax("US") == Decimal("350.99")
    book.discount_pct = Decimal("5")
    assert book.total_with_tax("UK") == Decimal("370.49")
    assert book.total_with_tax("EU") == Decimal("373.58")
    assert isinstance(book.total_with_tax("EU"), Decimal)


def test_unknown_region():
    try:
        make_book().total_with_tax("XX")
    except ValueError:
        return
    raise AssertionError("expected ValueError")
''')
variant("E5", "A", "No special tools; use the standard tools.")
variant("E5", "B", "`./outline FILE` prints every class, function and method of a Python file as `line: signature  # first docstring line` (methods indented).",
        needs=["outline"], files={"outline": HDR + 'exec python3 .mock/outline "$@"\n'})

# ---------------------------------------------------------------- E6
task("E6", """
# Task
In `shop/pricing.py` add a function `refund(order_id, amount)`. It fetches the order over
http (`GET {BASE_URL}/orders/{order_id}`, the response has a `total` string), logs one line
that mentions the order id and the amount, and returns the refund as a `Decimal` rounded to
cents: `Decimal(str(amount))`, capped at the order's total. If the response has an `error`
key it raises `ValueError`. `tests/test_e6.py` checks it.

`./lint` checks the project rules (silent with exit 0 means clean). Make `./lint` pass.
""")
extra("E6", "tests/test_e6.py", '''from decimal import Decimal

from shop import http, pricing
from shop.log import log


def test_refund():
    assert pricing.refund("A100", "30") == Decimal("30.00")
    assert pricing.refund("A200", Decimal("50")) == Decimal("19.99")
    assert pricing.refund("A100", 5) == Decimal("5.00")
    assert isinstance(pricing.refund("A100", 1), Decimal)


def test_refund_logs_and_timeout():
    http.calls.clear()
    log.records.clear()
    pricing.refund("A100", "1.50")
    assert any("A100" in r and "1.5" in r for r in log.records)
    method, url, timeout = http.calls[-1]
    assert method == "get" and url.endswith("/orders/A100") and timeout is not None


def test_refund_unknown():
    try:
        pricing.refund("ZZZ", 1)
    except ValueError:
        return
    raise AssertionError("expected ValueError")
''')
E6_LINT = {"lint": r123("terse")}
variant("E6", "A", "`./lint [FILE...]` (default: all of shop/) prints project-rule violations as `path:line:col RULE short-message`; silent with exit 0 means clean.",
        needs=["r123"], files=E6_LINT, setup='python3 .mock/r123 fixall shop/pricing.py\n')
variant("E6", "B", "`./lint [FILE...]` (default: all of shop/) prints project-rule violations as `path:line:col RULE short-message`; silent with exit 0 means clean.\nThe project rules are in `RULES.md`.",
        needs=["r123"], files={**E6_LINT, "RULES.md": """# Project rules

- R1: no `print(` in `shop/`; use `log.info(...)` from `shop.log`.
- R2: money values use `Decimal`, never float literals (in `shop/pricing.py`).
- R3: every `http.get(` / `http.post(` call passes `timeout=`.
"""}, setup='python3 .mock/r123 fixall shop/pricing.py\n')

# ---------------------------------------------------------------- E7
task("E7", """
# Task
Make the repository pass all of its checks: formatting, lint and typecheck. Do not change
behavior: the tests (`python3 -m tests`, including `tests/test_e7.py`, which describes the
intended behavior of `shop/typed.py`) must pass as well.
""")
extra("E7", "shop/typed.py", '''"""Small typed helpers."""


def area(w: int, h: int) -> int:
    return w * h


def pad(s: str, n: int) -> str:
    return s.ljust(n)


def code() -> str:
    return 42


def square() -> int:
    return area(3)


def banner() -> str:
    return pad("x", "5")
''')
extra("E7", "tests/test_e7.py", '''from shop import typed


def test_typed():
    assert typed.area(3, 4) == 12
    assert typed.code() == "42"
    assert typed.square() == 9
    assert typed.banner() == "x    "
''')
FMT = 'black --check shop 2>&1 | grep -v "^Oh no\\|^All done\\|files\\? would\\|files\\? left\\|^$" || true\nblack --check -q shop && echo "fmt: ok"\n'
LNT = 'ruff check --output-format concise --no-cache shop\n'
TYP = 'python3 .mock/typecheck_impl && echo "typecheck: ok"\n'
CHECK7 = (
    'rc=0\n'
    'echo "== fmt =="; black --check -q shop && echo "fmt: ok" || { black --check shop 2>&1 | grep "would reformat"; rc=1; }\n'
    'echo "== lint =="; ruff check --output-format concise --no-cache shop || rc=1\n'
    'echo "== typecheck =="; python3 .mock/typecheck_impl && echo "typecheck: ok" || rc=1\n'
    'exit $rc\n'
)
variant("E7", "A", "Three separate scripts, each prints findings (or `... ok`) and exits nonzero on failure: `./fmt-check` (black --check), `./lint` (ruff, default rules), `./typecheck` (a small type checker for annotated functions).",
        needs=["typecheck_impl"], files={"fmt-check": HDR + FMT, "lint": HDR + LNT, "typecheck": HDR + TYP})
variant("E7", "B", "`./check` runs all three checks (formatting with black --check, ruff lint with default rules, a small type checker for annotated functions) and prints the three groups under `== fmt ==`, `== lint ==`, `== typecheck ==`; exits nonzero when any fails.",
        needs=["typecheck_impl"], files={"check": HDR + CHECK7})

# ---------------------------------------------------------------- E8
task("E8", """
# Task
`./lint` reports violations of the project rules in `shop/`. Fix the R2 and R3 findings
in `shop/` (leave R1 findings alone). Do not change behavior: all tests must pass.
(R2 = float literal, R3 = http call without timeout.)
""")
legacy = ['"""Legacy steps (messy, chatty)."""', "from decimal import Decimal", "", "from shop import http", "", 'URL = "https://api.shop.example"', "", ""]
tests8 = ["from shop import http, legacy", "from decimal import Decimal", "", "", "def test_legacy():"]
for i in range(1, 36):
    legacy += [f"def step_{i}(value):", f'    print("step {i} start", value)', f'    print("step {i} detail")',
               f'    print("step {i} end")', f"    return value + {i}", "", ""]
    tests8.append(f"    assert legacy.step_{i}(1) == {1 + i}")
for i in range(1, 9):
    legacy += [f"def scaled_{i}(value):", f"    rate = 0.{i}5", "    return Decimal(value) * Decimal(str(rate))", "", ""]
    tests8.append(f"    assert legacy.scaled_{i}(100) == Decimal(100) * Decimal(\"0.{i}5\")")
for i in range(1, 9):
    legacy += [f"def fetch_{i}(key):", f'    return http.get(URL + "/users/" + key)', "", ""]
tests8 += ["    assert legacy.fetch_1(\"1\")[\"name\"] == \"Ada\"",
           "    http.calls.clear()", "    legacy.fetch_2(\"2\")", "    assert http.calls[-1][2] is not None", ""]
extra("E8", "shop/legacy.py", "\n".join(legacy).rstrip() + "\n")
extra("E8", "tests/test_legacy.py", "\n".join(tests8))
variant("E8", "A", "`./lint [PATH...]` prints every violation as `path:line:col RULE short-message` followed by the rule's full explanation (indented). Silent with exit 0 means clean.",
        needs=["r123"], files={"lint": r123("long", "R2_SCOPE=all ")})
variant("E8", "B", "`./lint [PATH...]` prints one summary line per rule (count and first three locations); `./lint --rule R1` lists the findings of one rule (at most 40 lines); `./explain RULE` prints a rule's full explanation. Silent with exit 0 means clean.",
        needs=["r123"], files={"lint": HDR + 'STYLE=summary R2_SCOPE=all exec python3 .mock/r123 lint "$@"\n',
                               "explain": HDR + 'exec python3 .mock/r123 explain "$@"\n'})

# ---------------------------------------------------------------- E9
task("E9", """
# Task
Project policy: never call `datetime.now()` without a timezone in `shop/` (that is,
`datetime.now(tz)` with a tz argument is fine). Make that an enforced rule: add it as a rule
that `./lint` reports. Rule files live in `rules/*.yaml` (see `rules/example.yaml` for the
format). `./lint` must report every violating call in `shop/` and nothing else
(`shop/clock.py` has examples of both).
""")
extra("E9", "shop/clock.py", '''"""Clock helpers."""
import datetime
from datetime import UTC, timezone


def bad_one():
    return datetime.now()


def bad_two():
    return datetime.datetime.now()


def bad_three():
    stamp = datetime.now( )
    return stamp


def bad_four(n):
    return [datetime.now() for _ in range(n)]


def good_tz():
    return datetime.now(timezone.utc)


def good_utc():
    return datetime.now(UTC)


def good_comment():
    # never call datetime.now() without a tz
    return None


def good_string():
    msg = "calling datetime.now() is forbidden"
    return msg
''')
variant("E9", "A", "`./lint` loads every `rules/*.yaml` and prints `path:line:col RULE-ID message` for each match; silent with exit 0 means clean. A rule file has keys `id`, `message`, `pattern` (a Python regex searched in each source line), optional `not-pattern` (a line matching it is skipped) and optional `paths` (list of globs, default `shop/**/*.py`). It reports a clear error for an invalid rule file.",
        needs=["rules_lint"], files={"lint": HDR + 'exec python3 .mock/rules_lint "$@"\n',
                                     "rules/example.yaml": 'id: no-todo\nmessage: "TODO comments must name an owner: TODO(name)"\npattern: "TODO(?!\\\\()"\npaths:\n  - "shop/**/*.py"\n'})

# ---------------------------------------------------------------- E10
task("E10", """
# Task
In `shop/report.py`, implement `user_names(ids, cache)`: it returns the display names of
the users with the given ids, in order, as a list of strings. Use `Cache.get_many(keys)` to
batch-load the users from `cache` (a `shop.cache.Cache` holding `User` objects by id);
an id that is not in the cache gives `"unknown"`. `tests/test_e10.py` checks it.
""")
extra("E10", "shop/report.py", '''"""Reports."""

from __future__ import annotations

from shop.cache import Cache


def user_names(ids: list[int], cache: Cache) -> list[str]:
    """Display names for ids ('unknown' when not cached)."""
    raise NotImplementedError
''')
extra("E10", "tests/test_e10.py", '''from shop.cache import Cache
from shop.models import User
from shop.report import user_names


def test_user_names():
    cache = Cache()
    cache.set(1, User(1, "Ada"))
    cache.set(2, User(2, "bob"))
    assert user_names([2, 99, 1], cache) == ["bob", "unknown", "Ada"]
    assert user_names([], cache) == []
''')
variant("E10", "A", "No special tools.")
variant("E10", "B", "`./check` statically reports unknown attributes of `Cache` and unknown names in `from shop.x import y` statements as `path:line: message`; silent with exit 0 means none found.",
        needs=["check10"], files={"check": HDR + 'exec python3 .mock/check10 "$@"\n'})

# make README of layout
print("variants:", sorted(p.parent.name + "/" + p.name for p in V.glob("*/*")))
