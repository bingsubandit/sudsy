import os
import sys
#      ^^^ unused-import
import json as json
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
^^^^^^^^^^^^^^^ no-wildcard-imports

import re
import re
#      ^^ redeclared-variable
#      ^^ unused-import


def f():
    import sys
#          ^^^ redeclared-variable
    return sys


for os in []:
#   ^^ shadowed-variable
    pass


class C:
    import unused_in_class


try:
    import t
except ImportError:
    t = None
