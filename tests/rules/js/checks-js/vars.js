var a = 1;
^^^^^^^^^^ no-var
let b = 2;
//  ^ prefer-const
let c = 3;
c = 4;
function f() {
  return arguments;
//       ^^^^^^^^^ prefer-rest-params
}
f.apply(undefined, b);
^^^^^^^^^^^^^^^^^^^^^ prefer-spread
f.apply(null, [1]);
