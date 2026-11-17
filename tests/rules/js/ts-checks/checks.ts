a?.b!;
^^^^^ no-non-null-asserted-optional-chain
(a?.b)!;
^^^^^^^ no-non-null-asserted-optional-chain
^^^^^^^^ no-unused-expressions
a?.b!.c;
^^^^^^^^ no-unused-expressions
function f<T extends unknown, U extends string, V extends any = 1>() {}
//         ^^^^^^^^^^^^^^^^^ no-unnecessary-type-constraint
//                                              ^^^^^^^^^^^^^^^^^ no-unnecessary-type-constraint
enum E { A = 0, B = 0, C = 0x1, D = 1, F = "x", G = "x", H = 2 }
//              ^^^^^ no-duplicate-enum-values
//                              ^^^^^ no-duplicate-enum-values
//                                              ^^^^^^^ no-duplicate-enum-values
let g: Function;
//     ^^^^^^^^ no-unsafe-function-type
let h: String | number;
//     ^^^^^^ no-wrapper-object-types
class K implements Function {}
//                 ^^^^^^^^ no-unsafe-function-type
interface Number {}
let i: Number;
a!;
^^^ no-unused-expressions
a as any;
^^^^^^^^^ no-unused-expressions
b!();
