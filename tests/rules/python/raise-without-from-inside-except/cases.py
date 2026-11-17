x = 1
try:
    pass
except OSError as e:
    if x:
        raise ValueError("nested")
#       ^^^^^^^^^^^^^^^^^^^^^^^^^^ raise-without-from-inside-except
    try:
        raise ValueError("try body")
    except KeyError:
        raise ValueError("inner handler")
#       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ raise-without-from-inside-except
    raise e
#   ^^^^^^^ verbose-raise
    raise ValueError() from e
    raise ValueError() from None

    def g():
        raise ValueError("function")


try:
    pass
except* OSError:
    raise ValueError
#   ^^^^^^^^^^^^^^^^ raise-without-from-inside-except
