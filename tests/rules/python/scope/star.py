from star import *
^^^^^^^^^^^^^^^^^^ no-wildcard-imports

__all__ = ["present", "from_star"]
#                     ^^^^^^^^^^^ undefined-local-with-import-star-usage
present = 1
print(undefined, present, len)
#     ^^^^^^^^^ undefined-local-with-import-star-usage


def nested():
    from other import *
#   ^^^^^^^^^^^^^^^^^^^ no-wildcard-imports
#   ^^^^^^^^^^^^^^^^^^^ undefined-local-with-nested-import-star-usage


def f(a: "Missing"):
#         ^^^^^^^ undefined-local-with-import-star-usage
    x: int
    print(x)
#         ^ undefined-variable
