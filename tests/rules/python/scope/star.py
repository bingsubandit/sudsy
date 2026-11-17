from star import *
^^^^^^^^^^^^^^^^^^ undefined-local-with-import-star

__all__ = ["present", "from_star"]
#         ^^^^^^^^^^^^^^^^^^^^^^^^ unsorted-dunder-all
#                     ^^^^^^^^^^^ undefined-local-with-import-star-usage
present = 1
print(undefined, present, len)
#     ^^^^^^^^^ undefined-local-with-import-star-usage


def nested():
    from other import *
#   ^^^^^^^^^^^^^^^^^^^ undefined-local-with-import-star
#   ^^^^^^^^^^^^^^^^^^^ undefined-local-with-nested-import-star-usage


def f(a: "Missing"):
#        ^^^^^^^^^ quoted-annotation
#         ^^^^^^^ undefined-local-with-import-star-usage
    x: int
    print(x)
#         ^ undefined-name
