fun a(x: Int): Int {
    return 1
    b()
//  ^^^ unreachable-code
    c()
}
fun d(x: List<Int>, y: Boolean) {
    for (i in x) { continue; e() }
//                           ^^^ unreachable-code
    while (true) { if (y) { break } }
    f()
    if (y) { return } else { throw Exception() }
    g()
//  ^^^ unreachable-code
}
fun h(x: Int) {
    try { return } catch (e: Exception) { }
//                 ^^^^^^^^^^^^^^^^^^^^^^^^ no-empty-catch
//                                      ^^^ unreachable-code
    i()
    when (x) {
        1 -> j()
        else -> return
    }
    k()
}
