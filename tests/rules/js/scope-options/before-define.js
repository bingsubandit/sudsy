export const q = g();
function g() {}
export const r = h();
//               ^ use-before-define
const h = () => 1;
