"""Plain test runner: python3 -m tests [module ...]"""

import importlib
import pathlib
import sys
import traceback

ROOT = pathlib.Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT))


def main(argv=None):
    names = argv or sorted(p.stem for p in (ROOT / "tests").glob("test_*.py"))
    passed = failed = 0
    for name in names:
        mod = importlib.import_module("tests." + name)
        for fn_name in sorted(n for n in dir(mod) if n.startswith("test_")):
            try:
                getattr(mod, fn_name)()
                passed += 1
                print(f"ok   {name}.{fn_name}")
            except Exception:
                failed += 1
                print(f"FAIL {name}.{fn_name}")
                traceback.print_exc()
    print(f"{passed} passed, {failed} failed")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
