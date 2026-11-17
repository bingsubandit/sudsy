def a(x, y)
  z = 1
# ^ unused-local-variable
  _w = y
# ^^ underscore-prefixed-variable-used
  puts _w
  x
end
[1].each { |i| i }
[1].each { |i| 1 }
#           ^ unused-block-argument
def b(*)
  [1].map { |_v| 1 }
end
def c(p)
#     ^ unused-method-argument
  1
end
def d(&blk)
  yield
end
x, _y = [1, 2]
puts x
puts "#{x}"
if (z ||= 1)
end
