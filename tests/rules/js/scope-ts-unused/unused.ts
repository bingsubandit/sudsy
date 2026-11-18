import { Used, Unused, OnlyTyped, Queried } from "m";
//             ^^^^^^ unused-variable
import type { TU } from "t";
//            ^^ unused-variable
export interface Exported { a: Used }
interface Local { b: string }
//        ^^^^^ unused-variable
type Alias = number; type UsedAlias = Alias;
enum E { A, B }
namespace N { export const n = 1 }
const x = 1; type Q = typeof x;
//    ^ unused-variable
//                ^ unused-variable
function f<T, U>(a: T): T { return a; }
//            ^ unused-variable
function over(a: number): void;
function over(a: string): void;
function over(a: any) {}
//            ^ unused-parameter
class C { constructor(private p: string, q: number) {} m(cb: (z: number) => void) {} }
//                                       ^ unused-parameter
//                                                       ^^ unused-parameter
declare function d(a: number): void;
let t: UsedAlias, o: OnlyTyped, qq: typeof Queried;
console.log(f, over, C, E, N, d, t, o, qq);
