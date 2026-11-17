fun f(a: Int, b: Int) {
    if (a == a) { g() }
//      ^^^^^^ no-self-compare
    if (true) { g() }
//      ^^^^ no-constant-condition
    while (true) { g() }
    if (1 == 2) { g() }
//      ^^^^^^ no-constant-comparison
    try { g() } catch (e: Exception) { }
//                                   ^^^ no-empty-block
}
