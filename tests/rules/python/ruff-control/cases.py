try:
    pass
except Exception:
^ try-except-pass
#      ^^^^^^^^^ blind-except
    pass

if a:
^ collapsible-if
#  ^ undefined-name
    if b:
#      ^ undefined-name
        pass

try:
    pass
except Exception:
^ try-except-pass
#      ^^^^^^^^^ blind-except
    pass

import pdb
^^^^^^^^^^ leftover-debugger
^^^^^^^^^^ unsorted-imports
#      ^^^ unused-import

def f(x):
    if x:
#   ^ needless-bool
        return True
    return False

if a:
#  ^ undefined-name
    x = 1
elif b:
^ if-with-same-arms
#    ^ undefined-name
    x = 1

f = open("a")
#   ^^^^^^^^^ open-file-without-with

with a:
^ nested-with
#    ^ undefined-name
    with b:
#        ^ undefined-name
        pass

raise Exception("x")
^^^^^^^^^^^^^^^^^^^^ raise-vanilla-class

if not isinstance(x, int):
    raise ValueError("x")
#   ^^^^^^^^^^^^^^^^^^^^^ type-check-without-type-error

def f(x):
    if x:
        return None
#       ^^^^^^^^^^^ explicit-return-none
    print(x)

x = 1
x
^ useless-expression

try:
    pass
except ValueError as e:
    raise e
#   ^^^^^^^ verbose-raise

def f():
    raise
#   ^^^^^ bare-raise-outside-handler

exec("x = 1")
^^^^^^^^^^^^^ exec-call

hasattr(x, "__call__")
^^^^^^^^^^^^^^^^^^^^^^ hasattr-call
