puts [1].map { |x| x }
^^^^^^^^^^^^^^^^^^^^^^ ambiguous-block-association
raise Exception, "m"
#     ^^^^^^^^^ raise-exception
raise StandardError
foo (1)
#   ^^^ parentheses-as-grouped-expression
foo (a ? b : c)
foo(1)
h[x.object_id]
^^^^^^^^^^^^^^ hash-compare-by-identity
h.key?(x.id)
1.times { f }
^^^^^^^^^^^^^ useless-times
2.times { f }
Struct.new(:hash, :name)
#          ^^^^^ struct-new-override
puts "#{x.to_s} #{y}"
#         ^^^^ redundant-string-coercion
puts x.to_s, y
#      ^^^^ redundant-string-coercion
x == x
^^^^^^ binary-operator-with-identical-operands
a && a
^^^^^^ binary-operator-with-identical-operands
a == b
