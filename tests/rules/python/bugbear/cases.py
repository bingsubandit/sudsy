import collections
from typing import Sequence
^^^^^^^^^^^^^^^^^^^^^^^^^^^ deprecated-import

x = a = b = 1


def f(a=[], b={}, c=set(), d=list(), e=collections.deque(), g=[x for x in a]):
#       ^^ mutable-argument-default
#             ^^ mutable-argument-default
#                   ^^^^^ function-call-in-default
#                   ^^^^^ mutable-argument-default
#                            ^^^^^^ function-call-in-default
#                            ^^^^^^ mutable-argument-default
#                            ^^^^^^ unnecessary-collection-call
#                                      ^^^^^^^^^^^^^^^^^^^ function-call-in-default
#                                      ^^^^^^^^^^^^^^^^^^^ mutable-argument-default
#                                                             ^^^^^^^^^^^^^^ mutable-argument-default
    pass


def immutable(a: Sequence[int] = [], b: tuple = (), c=(), d=frozenset(), e=bytearray()):
#                                                                          ^^^^^^^^^^^ function-call-in-default
    pass


assert False
^^^^^^^^^^^^ assert-false
assert False, "message"
^^^^^^^^^^^^^^^^^^^^^^^ assert-false
assert 0
getattr(x, "name")
^^^^^^^^^^^^^^^^^^ get-attr-with-constant
getattr(x, "__private")
getattr(x, "class")
getattr(x, "name", None)
setattr(x, "name", 1)
^^^^^^^^^^^^^^^^^^^^^ set-attr-with-constant
y = setattr(x, "name", 1)
setattr(x, "class", 1)
zip(a, b)
^^^^^^^^^ zip-without-explicit-strict
zip(a)
zip(a, b, strict=True)
try:
    pass
except (ValueError,):
#      ^^^^^^^^^^^^^ redundant-tuple-in-exception-handler
    pass
except (ValueError, TypeError):
    pass
