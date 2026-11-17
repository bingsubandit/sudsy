a = b = c = 1
"%" % a
^^^^^^^ percent-format-invalid-format
"%z" % a
^^^^^^^^ percent-format-unsupported-format-character
"%s %(x)s" % a
^^^^^^^^^^^^^^ percent-format-mixed-positional-and-named
"%(x)s" % (a,)
^^^^^^^^^^^^^^ percent-format-expected-mapping
"%s %s" % {"x": 1}
^^^^^^^^^^^^^^^^^^ percent-format-expected-sequence
^^^^^^^^^^^^^^^^^^ percent-format-positional-count-mismatch
"%(x)s" % {"x": 1, "y": 2}
^^^^^^^^^^^^^^^^^^^^^^^^^^ percent-format-extra-named-arguments
"%(x)s %(y)s" % {"x": 1}
^^^^^^^^^^^^^^^^^^^^^^^^ percent-format-missing-argument
"%s %s" % (a,)
^^^^^^^^^^^^^^ percent-format-positional-count-mismatch
"%*s" % {"x": 1}
^^^^^^^^^^^^^^^^ percent-format-expected-sequence
^^^^^^^^^^^^^^^^ percent-format-positional-count-mismatch
^^^^^^^^^^^^^^^^ percent-format-star-requires-sequence
"%s" % a
^^^^^^^^ printf-string-formatting
"%s %s" % (*a,)
"%(x)s" % {"x": 1, **a}
"%%" % ()
^^^^^^^^^ printf-string-formatting
b"%s" % (a, b)
