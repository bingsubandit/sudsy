a = b = c = 1
"{".format(a)
^^^^^^^^^^^^^ string-dot-format-invalid-format
"{} {0}".format(a)
^^^^^^^^^^^^^^^^^^ string-dot-format-mixing-automatic
"{} {}".format(a)
^^^^^^^^^^^^^^^^^ string-dot-format-missing-arguments
"{}".format(a, b)
^^^^^^^^^^^^^^^^^ string-dot-format-extra-positional-arguments
"{x}".format(y=1)
^^^^^^^^^^^^^^^^^ string-dot-format-extra-named-arguments
^^^^^^^^^^^^^^^^^ string-dot-format-missing-arguments
"{x}".format(x=1, y=2)
^^^^^^^^^^^^^^^^^^^^^^ string-dot-format-extra-named-arguments
"{:{}}".format(a, b)
^^^^^^^^^^^^^^^^^^^^ f-string
"{:{:{}}}".format(a, b, c)
^^^^^^^^^^^^^^^^^^^^^^^^^^ string-dot-format-invalid-format
"{} {x}".format(*a, **b)
"{}".format(a, *b)
"{0[a].b}".format(a)
^^^^^^^^^^^^^^^^^^^^ f-string
"{!}".format(a)
^^^^^^^^^^^^^^^ string-dot-format-invalid-format
"{0[}".format(a)
^^^^^^^^^^^^^^^^ string-dot-format-invalid-format
