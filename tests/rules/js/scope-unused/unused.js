import { a, b } from "m";
//       ^ unused-variable
import c from "c";
export const exported = 1;
const unused = 1, used = 2; let assigned = 1; assigned = 2;
//    ^^^^^^ unused-variable
//                                            ^^^^^^^^ unused-variable
function f(x, y, z) { return y; }
//       ^ unused-variable
//               ^ unused-parameter
function g(p, q) { return p; }
//       ^ unused-variable
//            ^ unused-parameter
function h({ d }, e = 1, ...rest) {}
//       ^ unused-variable
//           ^ unused-parameter
//                ^ unused-parameter
//                          ^^^^ unused-parameter
const fn = function self() { self(); };
function recursive() { recursive(); }
//       ^^^^^^^^^ unused-variable
let counter = 0; counter++; counter += 1;
//                          ^^^^^^^ unused-variable
let w = 0; w = w + 1; let v; v = () => v;
//         ^ unused-variable
//                           ^ unused-variable
try {} catch (err) {}
//  ^^ no-empty-block
//     ^^^^^^^^^^^^^^ no-empty-catch
//            ^^^ unused-variable
class Cls { static self = Cls; }
//    ^^^ unused-variable
const o = { set s(value) {} };
export function loop() { for (const k in o) { return; } }
var dup = 1; var dup = 2;
//               ^^^ unused-variable
export const nested = function nested() { var nested = 1; };
//                                            ^^^^^^ unused-variable
console.log(used, b, fn, c);
