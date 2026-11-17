def f():
    a, b = g()
#      ^ unused-unpacked-variable
#          ^ undefined-name
    return a

class A:
    x = []
#       ^^ mutable-class-default

import os as os
^^^^^^^^^^^^^^^ unsorted-imports
#      ^^^^^^^^ useless-import-alias

def f(a):
    with g() as a:
#        ^ undefined-name
#               ^ redefined-argument-from-local
#               ^ unused-variable
        pass

x = ["a" "b", "c"]
#    ^^^^^^^ implicit-concat-in-collection

__all__ = ["b", "a"]
#         ^^^^^^^^^^ unsorted-dunder-all

class A:
    __slots__ = ("b", "a")
#               ^^^^^^^^^^ unsorted-dunder-slots

import sys
^ unsorted-imports
#      ^^^ unused-import
import os
#      ^^ redefined-while-unused
#      ^^ unused-import

import functools
class A:
    @functools.cache
#   ^^^^^^^^^^^^^^^^ cached-method
    def f(self):
        pass

def f(x=list()):
#       ^^^^^^ function-call-in-default
#       ^^^^^^ mutable-argument-default
#       ^^^^^^ unnecessary-collection-call
    pass

import subprocess
^^^^^^^^^^^^^^^^^ unsorted-imports
subprocess.run(["ls"])
^^^^^^^^^^^^^^^^^^^^^^ subprocess-run-without-check

a = b = "x"
x = ", ".join([a, b])
#   ^^^^^^^^^^^^^^^^^ static-join

x = "%s" % y
#   ^^^^^^^^ printf-string-formatting
#          ^ undefined-name

x = "{}".format(y)
#   ^^^^^^^^^^^^^^ f-string
#               ^ undefined-name

import re
^^^^^^^^^ unsorted-imports
x = re.compile("a", re.I)
#                   ^^^^ regex-flag-alias
