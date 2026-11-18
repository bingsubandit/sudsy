export function f(o, _skip, a, b) {
//                             ^ unused-parameter
  const { x, ...rest } = o;
//              ^^^^ unused-variable
  const _hidden = 1;
  try { g(); } catch (err) { g(); }
  return a;
}
