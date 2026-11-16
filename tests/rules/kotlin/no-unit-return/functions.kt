fun a(): Unit {}
^^^ no-unit-return
fun b(x: Int = 1): Unit { return }
^^^ no-unit-return
fun c(): Int { return 1 }
fun d(): Unit = Unit
fun Unit.e() {}
fun f() {
    fun g(): Unit {}
//  ^^^ no-unit-return
}
