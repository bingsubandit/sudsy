from typing import Union
x: Union[int, str]
#  ^^^^^^^^^^^^^^^ union-annotation

from typing import Optional
x: Optional[int]
#  ^^^^^^^^^^^^^ optional-annotation

from typing import List
^^^^^^^^^^^^^^^^^^^^^^^ deprecated-import
x: List[int]
#  ^^^^ deprecated-typing-alias

from typing import Generator
^^^^^^^^^^^^^^^^^^^^^^^^^^^^ deprecated-import
x: Generator[int, None, None]
#            ^^^^^^^^^^^^^^^ default-type-arguments

def f(x: "int"):
#        ^^^^^ quoted-annotation
    pass

from typing import TypeAlias
X: TypeAlias = int
^^^^^^^^^^^^^^^^^^ type-alias-annotation

from typing import Generic, TypeVar
T = TypeVar("T")
class A(Generic[T]):
^ generic-class
    pass

from typing import TypeVar
T = TypeVar("T")
def f(x: T) -> T:
#   ^ generic-function
#   ^ redeclared-variable
    return x

import datetime
x = datetime.timezone.utc
#   ^^^^^^^^^^^^^^^^^^^^^ datetime-utc-alias

from typing_extensions import Self
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ deprecated-import

import sys
if sys.version_info >= (3, 8):
#  ^^^^^^^^^^^^^^^^^^^^^^^^^^ outdated-version-block
    pass

from typing import TypeVar
T = TypeVar("T")
class A:
#     ^ redeclared-variable
    def f(self: T) -> T:
#               ^ custom-type-var-return
        return self

from typing import Any
class A:
#     ^ redeclared-variable
    def __eq__(self, other: Any) -> bool:
#                           ^^^ any-eq-parameter
        return True

from typing import TypeVar
T = TypeVar("T", covariant=True)
^ type-var-name-variance
