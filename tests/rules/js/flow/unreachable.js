function a() { return 1; b(); var v; c(); function h() {} }
//                       ^^^^ unreachable-code
//                                   ^^^^ unreachable-code
function b() { throw e; ; d(); }
//                        ^^^^ unreachable-code
function c(x) { if (x) { return; } else { return; } e(); }
//                                                  ^^^^ unreachable-code
function d() { while (true) {} f(); }
//                          ^^ no-empty-block
//                             ^^^^ unreachable-code
function e(x) { for (;;) { if (x) break; } g(); }
function f(x) { switch (x) { case 1: return; default: return; } h(); }
//                                                              ^^^^ unreachable-code
function g(x) { switch (x) { case 1: return; } h(); }
function h(x) { try { return 1; } finally { i(); } j(); }
//                                                 ^^^^ unreachable-code
function i(x) { l: { break l; } k(); }
function j() { for (;;) { continue; k(); } }
//                                  ^^^^ unreachable-code
function k(x) { try { return 1; } catch (e) { } m(); }
//                                ^^^^^^^^^^^^^ no-empty-catch
//                                          ^^^ unreachable-code
function l(x) { do { break; } while (x); n(); }
function m() { return; let z = 1; const y = 2; class Z {} }
//                     ^^^^^^^^^^ unreachable-code
function n() { return; var w; let q; }
//                            ^^^^^^ unreachable-code
