use(undefinedThing, typeof maybe, typeof maybe.x, arguments);
^^^ undefined-variable
//  ^^^^^^^^^^^^^^ undefined-variable
//                                       ^^^^^ undefined-variable
//                                                ^^^^^^^^^ undefined-variable
function f(a, a2) { var a; return arguments; }
//                      ^ redeclared-variable
var x = 1; var x = 2; function x() {}
//             ^ redeclared-variable
//                             ^ redeclared-variable
function f2() {} function f2() {}
//                        ^^ redeclared-variable
try {} catch (e) { var e; }
//  ^^ no-empty-block
before(); function before() {} before2(); var before2 = 1;
^^^^^^ use-before-define
//                             ^^^^^^^ use-before-define
const s = s + 1; let t = () => t; class C { static p = C; q = C; } class D extends D {}
//        ^ use-before-define
//                                                                                 ^ use-before-define
for (var i of i) {} const { u = u } = {};
//            ^ use-before-define
//               ^^ no-empty-block
//                              ^ use-before-define
Promise.all([globalThis, hasOwnProperty]);
