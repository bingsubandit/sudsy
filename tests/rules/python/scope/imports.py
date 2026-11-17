import os
^ unsorted-imports
import sys
#      ^^^ unused-import
import json as json
#      ^^^^^^^^^^^^ useless-import-alias
import a.b
import a.c
#      ^^^ unused-import
from m import n, o as p
#             ^ unused-import
#                     ^ unused-import
from . import q
#             ^ unused-import
from __future__ import annotations
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ late-future-import
from k import *
^^^^^^^^^^^^^^^ undefined-local-with-import-star

import re
import re
#      ^^ redefined-while-unused
#      ^^ unused-import


def f():
    import sys
#          ^^^ redefined-while-unused
    return sys


for os in []:
#   ^^ import-shadowed-by-loop-var
    pass


class C:
    import unused_in_class


try:
    import t
except ImportError:
    t = None
