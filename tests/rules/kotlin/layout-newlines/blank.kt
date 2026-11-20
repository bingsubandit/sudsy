val a = 1
// about b
^ blank-line-between-declarations
val b = 2

@Deprecated("x")
fun c() {}

fun d(x: Int,
//    ^ newline-wrapping
      y: Int) {
//         ^ newline-wrapping
}
