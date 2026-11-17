"""Docstring: not code for E402."""
import os
^^^^^^^^^ unsorted-imports
#      ^^ unused-import
__version__ = "1"
x = 1
import sys
^^^^^^^^^^ module-import-not-at-top-of-file

break
^^^^^ break-outside-loop
continue
^^^^^^^^ continue-outside-loop
yield 1
^^^^^^^ yield-outside-function
await x
^^^^^^^ invalid-syntax
^^^^^^^ yield-outside-function


def f():
    class A:
        yield 1
#       ^^^^^^^ yield-outside-function

    try:
        pass
    except:
#   ^ default-except-not-last
#   ^ try-except-pass
#   ^^^^^^ bare-except
        pass
    except ValueError:
        pass
    print >> sys.stderr, "x"
#   ^^^^^^^^^^^^^^^^^^^ invalid-print-syntax
#   ^^^^^^^^^^^^^^^^^^^^^^^^ useless-expression


def g(l: "int", O=1, *I):
#     ^ ambiguous-variable-name
#        ^^^^^ quoted-annotation
#               ^ ambiguous-variable-name
#                     ^ ambiguous-variable-name
    l = 1
#   ^ ambiguous-variable-name
#   ^ unused-variable
    if x: pass
#       ^ multiple-statements-on-one-line-colon
    a = 1; b = 2;
#   ^ unused-variable
#        ^ multiple-statements-on-one-line-semicolon
#          ^ unused-variable
#               ^ useless-semicolon
    return {"k": 1, "k": 2, x: 1, x: 2}
#                   ^^^ no-duplicate-keys
#                                 ^ no-duplicate-keys


class I:
#     ^ ambiguous-class-name
    pass


def O():
#   ^ ambiguous-function-name
    pass


from __future__ import division
^ unsorted-imports
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ late-future-import
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ module-import-not-at-top-of-file


from __future__ import not_a_feature
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ late-future-import
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ module-import-not-at-top-of-file
#                      ^^^^^^^^^^^^^ future-feature-not-defined

*a, *b = range(3)
^^^^^^ multiple-starred-expressions
