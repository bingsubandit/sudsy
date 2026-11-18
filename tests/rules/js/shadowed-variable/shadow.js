var a = 1; let b = 2;
function f(a) { let b = 3; { const a = 4; } }
//         ^ shadowed-variable
//                  ^ shadowed-variable
//                                 ^ shadowed-variable
var c = function c() {}; var d = function (d) {};
//                                         ^ shadowed-variable
function outer() { var e = 1; function inner() { var e = 2; } }
//                                                   ^ shadowed-variable
function hoist() { var h = 1; } function h() {}
//                     ^ shadowed-variable
const t = () => { const late = 1; }; const late = 2;
class K { m() { var K = 1; } }
//                  ^ shadowed-variable
