import pprint
from pprint import pprint as pp

a = b = c = f = 1

print(1)
^^^^^^^^ no-print
print()
^^^^^^^ no-print
print("x", file=f)
print("x", file=None)
^^^^^^^^^^^^^^^^^^^^^ no-print
pprint.pprint(1)
^^^^^^^^^^^^^^^^ no-print
pp(2)
^^^^^ no-print
pprint(3)
x = dict()
#   ^^^^^^ unnecessary-collection-call
x = dict(a=1)
#   ^^^^^^^^^ unnecessary-collection-call
x = dict(**a)
x = dict(a, b=1)
x = list()
#   ^^^^^^ unnecessary-collection-call
x = tuple()
#   ^^^^^^^ unnecessary-collection-call
x = list(a)
x = a.startswith("a") or a.startswith("b")
#   ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ multiple-starts-ends-with
x = a.endswith("a") or a.endswith(("b", "c")) or c
#   ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ multiple-starts-ends-with
x = a.startswith("a") or b.startswith("b")
x = a.startswith("a") and a.startswith("b")
x = a.startswith("a") or a.endswith("b")
if a:
    pass
#   ^^^^ unnecessary-placeholder
    pass
#   ^^^^ unnecessary-placeholder
else:
    ...
#   ^^^ unnecessary-placeholder
    x = 1


def f():
    """doc"""
    ...
#   ^^^ unnecessary-placeholder


class C:
    pass
