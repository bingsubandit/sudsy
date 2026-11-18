__all__ = ["present", "missing"]
#                     ^^^^^^^^^ undefined-export
present = 1
print(undefined, present, len)
#     ^^^^^^^^^ undefined-variable


def reads_before_assignment():
    print(present)
#         ^^^^^^^ use-before-define
    present = 2
#   ^^^^^^^ unused-variable


def handled():
    try:
        name_error_is_handled
#       ^^^^^^^^^^^^^^^^^^^^^ no-useless-expression
    except NameError:
        pass
    del present
#       ^^^^^^^ undefined-variable
    del not_bound
#       ^^^^^^^^^ undefined-variable


def annotations(a: "Missing", b: "bad syntax here"):
#                   ^^^^^^^ undefined-variable
#                                ^^^^^^^^^^^^^^^^^ forward-annotation-syntax-error
    x: int
    print(x)
#         ^ undefined-variable


class K:
    attr = 1
    print(attr, __qualname__)

    def method(self):
        return attr
#              ^^^^ undefined-variable
