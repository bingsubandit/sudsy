if (a === a) {} if (a.b < a.b) {} if (f() === f()) {} if (a === b) {}
//  ^^^^^^^ no-self-compare
//           ^^ no-empty-block
//                  ^^^^^^^^^ no-self-compare
//                             ^^ no-empty-block
//                                    ^^^^^^^^^^^ no-self-compare
//                                                 ^^ no-empty-block
//                                                                 ^^ no-empty-block
if (1 === 2) {} if (a === 1) {}
//  ^^^^^^^ no-constant-comparison
//           ^^ no-empty-block
//                           ^^ no-empty-block
if (true) {} while (0) {} while (true) {} for (;;) {} var c = "x" ? 1 : 2, d = a ? 1 : 2;
//  ^^^^ no-constant-condition
//        ^^ no-empty-block
//                  ^ no-constant-condition
//                     ^^ no-empty-block
//                                     ^^ no-empty-block
//                                        ^^^^^^^^^^^ no-unreachable
//                                                 ^^ no-empty-block
//                                                            ^^^ no-constant-condition
