def f(a, b)
  if true then g() end
#    ^^^^ no-constant-condition
  while true do g() end
  if a = b then g() end
#    ^^^^^ no-assignment-in-condition
end
