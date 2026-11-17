switch (a) { case 1: break; case 1: break; case "x": break; case 'x': break; case b + 1: break; case b+1: break; default: }
//                          ^^^^^^^^^^^^^^ no-duplicate-case
//                                                                                              ^^^^^^^^^^^^^^^^ no-duplicate-case
if (a) {} else if (b) {} else if (a) {}
//     ^^ no-empty-block
//                    ^^ no-empty-block
//                                ^ no-dupe-else-if
//                                   ^^ no-empty-block
if (a || b) {} else if (a) {} else if (b && c) {}
//          ^^ no-empty-block
//                      ^ no-dupe-else-if
//                         ^^ no-empty-block
//                                     ^^^^^^ no-dupe-else-if
//                                             ^^ no-empty-block
if (a && b) {} else if (a) {} else if (b && a) {}
//          ^^ no-empty-block
//                         ^^ no-empty-block
//                                     ^^^^^^ no-dupe-else-if
//                                             ^^ no-empty-block
if (a) { if (a) {} } else if (c) {} else { if (c) {} }
//              ^^ no-empty-block
//                               ^^ no-empty-block
//                                                ^^ no-empty-block
