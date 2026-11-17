x = :true
#   ^^^^^ boolean-symbol
y = :ok
z = 'a #{b}'
#   ^^^^^^^^ interpolation-check
"a #{1} #{x} #{[:a]}"
#    ^ literal-in-interpolation
#              ^^^^ literal-in-interpolation
/(?<a>x)(y)/
^^^^^^^^^^^^ mixed-regexp-capture-types
/(?<a>x)(?:y)/
a = a
^^^^^ self-assignment
@b ||= @b
^^^^^^^^^ self-assignment
c = d
^ unused-local-variable
f == 0.1
^^^^^^^^ float-comparison
f == 0
begin
  x
end while y
#   ^^^^^ post-condition-loop
case x
when 1
^^^^^^ empty-when
when 2 then z
end
begin
  1
rescue Exception
^^^^^^^^^^^^^^^^ rescue-exception
^^^^^^^^^^^^^^^^ suppressed-exception
end
begin
  1
rescue
^^^^^^ suppressed-exception
end
# rubocop:disable Foo/Bar
^^^^^^^^^^^^^^^^^^^^^^^^^ unknown-cop-name
h = { true: 1 }
^ unused-local-variable
#     ^^^^^ boolean-symbol
