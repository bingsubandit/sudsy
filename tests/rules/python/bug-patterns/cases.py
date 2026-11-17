def f(a, b):
    if a == a:
#      ^^^^^^ no-self-compare
        pass
    if True:
#      ^^^^ no-constant-condition
        pass
    while True:
        pass
    if 1 == 2:
#      ^^^^^^ no-constant-comparison
        pass
    try:
        pass
    except Exception:
        pass
