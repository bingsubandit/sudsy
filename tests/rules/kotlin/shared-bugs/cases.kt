fun f(a: Int) {
    if (a == a) { }
//      ^^^^^^ no-self-compare
//              ^^^ no-empty-block
    if (true) { g() }
//      ^^^^ no-constant-condition
    when (a) { 1 -> g(); 1 -> g() }
//                       ^^^^^^^^ no-duplicate-case
    try { g() } catch (e: Exception) { }
//              ^^^^^^^^^^^^^^^^^^^^^^^^ no-empty-catch
}
