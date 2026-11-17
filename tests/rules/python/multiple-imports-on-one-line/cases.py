import os, sys
^ unsorted-imports
^^^^^^^^^^^^^^ multiple-imports-on-one-line
import os
import os.path as p, re
^^^^^^^^^^^^^^^^^^^^^^^ multiple-imports-on-one-line
from a import b, c
