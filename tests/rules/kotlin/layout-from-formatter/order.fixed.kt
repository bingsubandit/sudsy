import b.B
import a.A

fun f(x: Int) = when (x) {
    1 -> {
        foo()
    }

    2 -> bar()
    else -> baz()
}

// one line
val a = 1
