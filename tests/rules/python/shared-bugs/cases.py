if a == a: pass
#  ^ undefined-name
#  ^^^^^^ no-self-compare
#       ^ undefined-name
if 1 == 2: pass
#  ^^^^^^ no-constant-comparison
if True: pass
#  ^^^^ no-constant-condition
while 1: pass
if (b := f()): pass
#        ^ undefined-name
d = {"k": 1, "k": 2, **d, **d}
#            ^^^ no-duplicate-keys
#                      ^ undefined-name
#                           ^ undefined-name
match a:
#     ^ undefined-name
    case 1: pass
    case 1: pass
#   ^^^^^^^^^^^^ no-duplicate-case
