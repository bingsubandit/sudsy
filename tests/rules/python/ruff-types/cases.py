from typing import Union
^^^^^^^^^^^^^^^^^^^^^^^^ unsorted-imports
x: Union[int, str]
#  ^^^^^^^^^^^^^^^ union-annotation

from typing import Optional
^^^^^^^^^^^^^^^^^^^^^^^^^^^ unsorted-imports
x: Optional[int]
#  ^^^^^^^^^^^^^ optional-annotation

from typing import List
^^^^^^^^^^^^^^^^^^^^^^^ deprecated-import
^^^^^^^^^^^^^^^^^^^^^^^ unsorted-imports
x: List[int]
#  ^^^^ deprecated-typing-alias

from typing import Generator
^^^^^^^^^^^^^^^^^^^^^^^^^^^^ deprecated-import
^^^^^^^^^^^^^^^^^^^^^^^^^^^^ unsorted-imports
x: Generator[int, None, None]
#            ^^^^^^^^^^^^^^^ default-type-arguments

def f(x: "int"):
#        ^^^^^ quoted-annotation
    pass

from typing import TypeAlias
^^^^^^^^^^^^^^^^^^^^^^^^^^^^ unsorted-imports
X: TypeAlias = int
^^^^^^^^^^^^^^^^^^ type-alias-annotation

from typing import Generic, TypeVar
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ unsorted-imports
T = TypeVar("T")
class A(Generic[T]):
^ generic-class
    pass

from typing import TypeVar
^^^^^^^^^^^^^^^^^^^^^^^^^^ unsorted-imports
T = TypeVar("T")
def f(x: T) -> T:
#   ^ generic-function
    return x

import datetime
^^^^^^^^^^^^^^^ unsorted-imports
x = datetime.timezone.utc
#   ^^^^^^^^^^^^^^^^^^^^^ datetime-utc-alias

from typing_extensions import Self
^ unsorted-imports
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ deprecated-import
#                             ^^^^ unused-import

import sys
if sys.version_info >= (3, 8):
#  ^^^^^^^^^^^^^^^^^^^^^^^^^^ outdated-version-block
    pass

from typing import TypeVar
^^^^^^^^^^^^^^^^^^^^^^^^^^ unsorted-imports
T = TypeVar("T")
class A:
    def f(self: T) -> T:
#               ^ custom-type-var-return
        return self

from typing import Any
^^^^^^^^^^^^^^^^^^^^^^ unsorted-imports
class A:
    def __eq__(self, other: Any) -> bool:
#                           ^^^ any-eq-parameter
        return True

from typing import TypeVar
^^^^^^^^^^^^^^^^^^^^^^^^^^ unsorted-imports
T = TypeVar("T", covariant=True)
^ type-var-name-variance
