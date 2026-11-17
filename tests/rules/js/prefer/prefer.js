var a = 1;
^^^^^^^^^^ no-var
let b = 1;
//  ^ prefer-const
let c = 1;
c = 2;
let d;
d = 1;
^ prefer-const
let e, f;
[e, f] = g();
 ^ prefer-const
//  ^ prefer-const
for (let h of i) {}
//       ^ prefer-const
for (let j = 0; j < 3; ) {}
function k() {
  let l;
  l = m();
//^ prefer-const
  return arguments[0] + arguments.length;
//       ^^^^^^^^^ prefer-rest-params
}
n.apply(undefined, o);
^^^^^^^^^^^^^^^^^^^^^ prefer-spread
p.q.apply(p, o);
^^^^^^^^^^^^^^^ prefer-spread
n.apply(null, [1]);
r + 1;
^^^^^^ no-unused-expressions
s.t;
^^^^ no-unused-expressions
u();
void 0;
"use strict";
^^^^^^^^^^^^^ no-unused-expressions
