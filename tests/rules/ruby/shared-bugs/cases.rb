if a == a
  b
end
h = { a: 1, a: 2 }
#           ^^ no-duplicate-keys
case x
when 1 then y
when 1 then z
^^^^^^^^^^^^^ no-duplicate-case
end
