x = y = 1
if (x, y):
^^ if-tuple
    pass
elif (x, y):
^ if-with-same-arms
^^^^ if-tuple
    pass
elif (x):
^ if-with-same-arms
    pass
if (x):
    pass
