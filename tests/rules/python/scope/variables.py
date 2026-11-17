def f(items):
    unused = 1
#   ^^^^^^ unused-variable
    used = 2
    a, b = items
#   ^ unused-unpacked-variable
#      ^ unused-unpacked-variable
    c = d = 3
#   ^ unused-variable
#       ^ unused-variable
    (e := 4)
#    ^ unused-variable
    with items as w:
#                 ^ unused-variable
        pass
    for i in items:
        pass
    try:
        pass
    except ValueError as err:
#                        ^^^ unused-variable
        pass
    _ = 5
    annotated: int
#   ^^^^^^^^^ unused-annotation
    kept: int = 6
    __tracebackhide__ = True
    return used, kept


def uses_locals():
    hidden = 1
    return locals()


def global_names():
    global counter
    counter = 1

    def inner():
        nonlocal captured
        captured = 2

    captured = 0
    return inner


lam = lambda: (walrus := 1)
#              ^^^^^^ unused-variable
