A = B = 1
try:
    pass
except:
^ default-except-not-last
^ try-except-pass
^^^^^^ bare-except
    pass
except A:
    pass
try:
    pass
except (A, B):
    pass
